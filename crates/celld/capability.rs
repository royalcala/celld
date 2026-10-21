// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! Opaque capability tokens for cross-isolate RPC.
//!
//! A handle exported from one isolate is identified by a token minted here, not
//! by an enumerable entry id or by a cell scope supplied by JavaScript. The
//! token binds the owner route (kind, owner, node), the owner incarnation
//! (`generation`), the handle id and caller flags under an HMAC. A receiver can
//! only use a handle it was given, and a token from a previous incarnation of
//! the owner cannot alias a new entry.
//!
//! The registry is deliberately a pure in-memory structure: the isolate owns
//! the actual target, while this records the reference count and the claims
//! needed to release it. The host wraps the registry; it is not shared between
//! threads here.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use std::fmt;

type HmacSha256 = Hmac<Sha256>;

static OWN_NODE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static INCARNATION: std::sync::OnceLock<u64> = std::sync::OnceLock::new();

/// Installs this process's fleet identity before any isolate mints a token.
/// The incarnation makes a token from a previous process (same node name)
/// stale even when its isolate heap ids repeat.
pub fn set_identity(node: &str) {
    let _ = OWN_NODE.set(node.to_owned());
    let mut bytes = [0u8; 8];
    let _ = getrandom::fill(&mut bytes);
    let _ = INCARNATION.set(u64::from_le_bytes(bytes));
}

/// The node that minted this process's tokens (empty in tests).
pub fn own_node() -> &'static str {
    OWN_NODE.get().map(String::as_str).unwrap_or("")
}

/// A token generation for one isolate heap. Distinct processes and distinct
/// isolate incarnations produce distinct values.
pub fn generation_for(heap: &str) -> u64 {
    let incarnation = *INCARNATION.get().unwrap_or(&0);
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ incarnation;
    for byte in heap.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Bumped only when the token layout changes.
const TOKEN_VERSION: u8 = 2;
/// Bit 0 is the caller-visible "callable target" marker set by `__cap_mint`.
pub const FLAG_CALLABLE: u8 = 0b01;
/// Bit 1 names a handle that is a row of the owner's persistent-stub table, so
/// an operation on it replays `[restore]` instead of looking up a live target.
pub const FLAG_RESTORE: u8 = 0b10;
/// Bit 2 narrows a restore handle to a facet: the owner replays `[restore]`
/// through the facet that the row's spec names, because a facet has no
/// published route of its own.
pub const FLAG_FACET_RESTORE: u8 = 0b100;
/// Domain separation for the capability key derived from a fleet secret.
const KEY_DOMAIN: &[u8] = b"celld-capability-key-v1";
/// Domain separation for the token MAC.
const MAC_DOMAIN: &[u8] = b"celld-capability-token-v1";
const MAC_LEN: usize = 32;
const MAX_OWNER_LEN: usize = 512;
const MAX_NODE_LEN: usize = 256;
const MAX_TOKEN_LEN: usize = 8 * 1024;

/// What owns the target a capability points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityKind {
    /// A target in a Durable Object cell. Routable by cell scope.
    Cell = 1,
    /// A target in a stateless isolate (a worker or a callback owner).
    Stateless = 2,
    /// A target in a facet realm.
    Facet = 3,
    /// A target in a Worker Loader (dynamic) worker.
    Loaded = 4,
    /// A service entrypoint or loopback target.
    Service = 5,
}

impl CapabilityKind {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn from_u8(value: u8) -> Result<Self, CapabilityError> {
        match value {
            1 => Ok(Self::Cell),
            2 => Ok(Self::Stateless),
            3 => Ok(Self::Facet),
            4 => Ok(Self::Loaded),
            5 => Ok(Self::Service),
            other => Err(CapabilityError::UnknownKind(other)),
        }
    }
}

/// The authenticated contents of a capability token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityClaims {
    /// Who owns the target.
    pub kind: CapabilityKind,
    /// Cell scope, facet scope, heap id, or service name, by kind.
    pub owner: String,
    /// Routing scope when it differs from `owner` (a facet whose root owns
    /// the lease). Empty means route by `owner`.
    pub route: String,
    /// Node that owns the target. Empty only for tests.
    pub node: String,
    /// Owner incarnation. A token from an older incarnation is stale.
    pub generation: u64,
    /// Handle id within the owner.
    pub handle: u64,
    /// Reserved for caller-visible flags (dispose-only, durable, ...).
    pub flags: u8,
}

impl CapabilityClaims {
    /// Builds claims for a cell-owned target.
    pub fn cell(
        owner: impl Into<String>,
        node: impl Into<String>,
        generation: u64,
        handle: u64,
    ) -> Self {
        Self {
            kind: CapabilityKind::Cell,
            owner: owner.into(),
            route: String::new(),
            node: node.into(),
            generation,
            handle,
            flags: 0,
        }
    }
}

/// Errors from minting, verifying or tracking a capability.
#[derive(Debug, PartialEq, Eq)]
pub enum CapabilityError {
    /// The key derivation secret was empty.
    EmptySecret,
    /// The token is not valid base64url or is truncated.
    Malformed,
    /// The token's MAC did not verify.
    BadMac,
    /// The token layout version is not supported.
    UnknownVersion(u8),
    /// The target kind is not recognized.
    UnknownKind(u8),
    /// The owner string exceeds the token bound.
    OwnerTooLong,
    /// The node string exceeds the token bound.
    NodeTooLong,
    /// The encoded token exceeds the token bound.
    TokenTooLong,
    /// The registry already has this handle.
    DuplicateHandle,
    /// The registry does not have this handle.
    UnknownHandle,
    /// A handle was registered with no references.
    InvalidRefs,
    /// The reference count would underflow.
    RefUnderflow,
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySecret => write!(f, "capability key secret is empty"),
            Self::Malformed => write!(f, "capability token is malformed"),
            Self::BadMac => write!(f, "capability token failed authentication"),
            Self::UnknownVersion(version) => {
                write!(f, "unsupported capability token version {version}")
            }
            Self::UnknownKind(kind) => write!(f, "unsupported capability kind {kind}"),
            Self::OwnerTooLong => write!(f, "capability owner is too long"),
            Self::NodeTooLong => write!(f, "capability node is too long"),
            Self::TokenTooLong => write!(f, "capability token is too long"),
            Self::DuplicateHandle => write!(f, "capability handle is already registered"),
            Self::UnknownHandle => write!(f, "capability handle is not registered"),
            Self::InvalidRefs => write!(f, "capability handle needs at least one reference"),
            Self::RefUnderflow => write!(f, "capability reference count underflow"),
        }
    }
}

impl std::error::Error for CapabilityError {}

/// HMAC key for capability tokens.
///
/// In a fleet this is derived from the fleet's peer secret so every node can
/// verify a token, while it remains distinct from the request-signing key.
#[derive(Clone)]
pub struct CapabilityKey([u8; 32]);

impl fmt::Debug for CapabilityKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CapabilityKey(..)")
    }
}

impl CapabilityKey {
    /// Uses a raw key. Intended for tests and for callers that already hold a
    /// dedicated capability secret.
    pub fn new(key: [u8; 32]) -> Self {
        Self(key)
    }

    /// Derives the capability key from a fleet secret.
    pub fn derive(secret: &[u8]) -> Result<Self, CapabilityError> {
        if secret.is_empty() {
            return Err(CapabilityError::EmptySecret);
        }
        let mut mac =
            HmacSha256::new_from_slice(secret).map_err(|_| CapabilityError::EmptySecret)?;
        mac.update(KEY_DOMAIN);
        let derived = mac.finalize().into_bytes();
        let mut key = [0_u8; 32];
        key.copy_from_slice(&derived);
        Ok(Self(key))
    }

    /// Mints a token for `claims`.
    pub fn mint(&self, claims: &CapabilityClaims) -> Result<String, CapabilityError> {
        let payload = encode_claims(claims)?;
        self.sign_payload(&payload)
    }

    fn sign_payload(&self, payload: &[u8]) -> Result<String, CapabilityError> {
        let mut token = payload.to_vec();
        let mac = self.mac(payload)?;
        token.extend_from_slice(&mac);
        if token.len() > MAX_TOKEN_LEN {
            return Err(CapabilityError::TokenTooLong);
        }
        Ok(URL_SAFE_NO_PAD.encode(token))
    }

    fn mac(&self, payload: &[u8]) -> Result<[u8; MAC_LEN], CapabilityError> {
        let mut mac =
            HmacSha256::new_from_slice(&self.0).map_err(|_| CapabilityError::EmptySecret)?;
        mac.update(MAC_DOMAIN);
        mac.update(payload);
        let bytes = mac.finalize().into_bytes();
        let mut out = [0_u8; MAC_LEN];
        out.copy_from_slice(&bytes);
        Ok(out)
    }

    /// Verifies a token and returns its authenticated claims.
    pub fn verify(&self, token: &str) -> Result<CapabilityClaims, CapabilityError> {
        if token.len() > MAX_TOKEN_LEN {
            return Err(CapabilityError::TokenTooLong);
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(token.as_bytes())
            .map_err(|_| CapabilityError::Malformed)?;
        if bytes.len() <= MAC_LEN {
            return Err(CapabilityError::Malformed);
        }
        let (payload, mac) = bytes.split_at(bytes.len() - MAC_LEN);
        let mut verifier =
            HmacSha256::new_from_slice(&self.0).map_err(|_| CapabilityError::EmptySecret)?;
        verifier.update(MAC_DOMAIN);
        verifier.update(payload);
        verifier
            .verify_slice(mac)
            .map_err(|_| CapabilityError::BadMac)?;
        parse_claims(payload)
    }
}

fn encode_claims(claims: &CapabilityClaims) -> Result<Vec<u8>, CapabilityError> {
    let owner = claims.owner.as_bytes();
    let route = claims.route.as_bytes();
    let node = claims.node.as_bytes();
    if owner.len() > MAX_OWNER_LEN {
        return Err(CapabilityError::OwnerTooLong);
    }
    if node.len() > MAX_NODE_LEN {
        return Err(CapabilityError::NodeTooLong);
    }
    let owner_len = u16::try_from(owner.len()).map_err(|_| CapabilityError::OwnerTooLong)?;
    let node_len = u16::try_from(node.len()).map_err(|_| CapabilityError::NodeTooLong)?;
    let route_len = u16::try_from(route.len()).map_err(|_| CapabilityError::OwnerTooLong)?;

    let mut payload =
        Vec::with_capacity(1 + 1 + 1 + 8 + 8 + 2 + owner.len() + 2 + node.len() + 2 + route.len());
    payload.push(TOKEN_VERSION);
    payload.push(claims.kind.as_u8());
    payload.push(claims.flags);
    payload.extend_from_slice(&claims.generation.to_le_bytes());
    payload.extend_from_slice(&claims.handle.to_le_bytes());
    payload.extend_from_slice(&owner_len.to_le_bytes());
    payload.extend_from_slice(owner);
    payload.extend_from_slice(&node_len.to_le_bytes());
    payload.extend_from_slice(node);
    payload.extend_from_slice(&route_len.to_le_bytes());
    payload.extend_from_slice(route);
    Ok(payload)
}

fn parse_claims(payload: &[u8]) -> Result<CapabilityClaims, CapabilityError> {
    let mut cursor = 0_usize;
    let version = take_u8(payload, &mut cursor)?;
    if version != TOKEN_VERSION {
        return Err(CapabilityError::UnknownVersion(version));
    }
    let kind = CapabilityKind::from_u8(take_u8(payload, &mut cursor)?)?;
    let flags = take_u8(payload, &mut cursor)?;
    let generation = take_u64(payload, &mut cursor)?;
    let handle = take_u64(payload, &mut cursor)?;
    let owner = take_string(payload, &mut cursor, MAX_OWNER_LEN)?;
    let node = take_string(payload, &mut cursor, MAX_NODE_LEN)?;
    let route = take_string(payload, &mut cursor, MAX_OWNER_LEN)?;
    if cursor != payload.len() {
        return Err(CapabilityError::Malformed);
    }
    Ok(CapabilityClaims {
        kind,
        owner,
        route,
        node,
        generation,
        handle,
        flags,
    })
}

fn take_u8(payload: &[u8], cursor: &mut usize) -> Result<u8, CapabilityError> {
    let value = *payload.get(*cursor).ok_or(CapabilityError::Malformed)?;
    *cursor += 1;
    Ok(value)
}

fn take_u64(payload: &[u8], cursor: &mut usize) -> Result<u64, CapabilityError> {
    let end = cursor.checked_add(8).ok_or(CapabilityError::Malformed)?;
    let bytes = payload
        .get(*cursor..end)
        .ok_or(CapabilityError::Malformed)?;
    let mut value = [0_u8; 8];
    value.copy_from_slice(bytes);
    *cursor = end;
    Ok(u64::from_le_bytes(value))
}

fn take_string(
    payload: &[u8],
    cursor: &mut usize,
    max_len: usize,
) -> Result<String, CapabilityError> {
    let end = cursor.checked_add(2).ok_or(CapabilityError::Malformed)?;
    let len_bytes = payload
        .get(*cursor..end)
        .ok_or(CapabilityError::Malformed)?;
    let mut len = [0_u8; 2];
    len.copy_from_slice(len_bytes);
    let len = usize::from(u16::from_le_bytes(len));
    if len > max_len {
        return Err(CapabilityError::Malformed);
    }
    *cursor = end;
    let end = cursor.checked_add(len).ok_or(CapabilityError::Malformed)?;
    let bytes = payload
        .get(*cursor..end)
        .ok_or(CapabilityError::Malformed)?;
    *cursor = end;
    String::from_utf8(bytes.to_vec()).map_err(|_| CapabilityError::Malformed)
}

/// Registry entry for one exported handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandleState {
    /// Authenticated claims. `handle` matches the map key.
    pub claims: CapabilityClaims,
    /// Live references. Reaches zero when every receiver disposed.
    pub refs: u32,
}

/// In-memory handle table for one owner isolate.
#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    handles: HashMap<u64, HandleState>,
}

impl CapabilityRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a handle with at least one reference.
    pub fn register(&mut self, claims: CapabilityClaims, refs: u32) -> Result<(), CapabilityError> {
        if refs == 0 {
            return Err(CapabilityError::InvalidRefs);
        }
        if self.handles.contains_key(&claims.handle) {
            return Err(CapabilityError::DuplicateHandle);
        }
        self.handles
            .insert(claims.handle, HandleState { claims, refs });
        Ok(())
    }

    /// Adds a reference to a live handle and returns the new count.
    pub fn retain(&mut self, handle: u64) -> Result<u32, CapabilityError> {
        let entry = self
            .handles
            .get_mut(&handle)
            .ok_or(CapabilityError::UnknownHandle)?;
        entry.refs = entry
            .refs
            .checked_add(1)
            .ok_or(CapabilityError::RefUnderflow)?;
        Ok(entry.refs)
    }

    /// Drops one reference. Returns `true` when the handle was removed.
    pub fn release(&mut self, handle: u64) -> Result<bool, CapabilityError> {
        let entry = self
            .handles
            .get_mut(&handle)
            .ok_or(CapabilityError::UnknownHandle)?;
        if entry.refs == 0 {
            return Err(CapabilityError::RefUnderflow);
        }
        entry.refs -= 1;
        if entry.refs == 0 {
            self.handles.remove(&handle);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Looks up a live handle.
    pub fn get(&self, handle: u64) -> Option<&HandleState> {
        self.handles.get(&handle)
    }

    /// Number of live handles.
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

/// Reference counts for capabilities exported across isolates. Keyed by the
/// owner and handle rather than by token: every token for the same capability
/// shares one count, and only the last release asks the owner to dispose it.
#[derive(Debug, Default)]
pub struct CapabilityRefs {
    entries: HashMap<(String, u64), u32>,
}

impl CapabilityRefs {
    /// Add one reference and return the new count.
    pub fn retain(&mut self, owner: &str, handle: u64) -> u32 {
        let count = self.entries.entry((owner.to_owned(), handle)).or_default();
        *count = count.saturating_add(1);
        *count
    }

    /// Drop one reference. Returns `Some(true)` when the last reference was
    /// dropped, `Some(false)` while references remain, and `None` when the
    /// capability was never registered or was already released.
    pub fn release(&mut self, owner: &str, handle: u64) -> Option<bool> {
        let key = (owner.to_owned(), handle);
        let count = self.entries.get_mut(&key)?;
        if *count > 1 {
            *count -= 1;
            Some(false)
        } else {
            self.entries.remove(&key);
            Some(true)
        }
    }

    /// The current reference count, if any.
    pub fn count(&self, owner: &str, handle: u64) -> Option<u32> {
        self.entries.get(&(owner.to_owned(), handle)).copied()
    }

    /// Number of tracked capabilities.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no capabilities are tracked.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> CapabilityKey {
        CapabilityKey::new([7_u8; 32])
    }

    fn claims() -> CapabilityClaims {
        CapabilityClaims {
            kind: CapabilityKind::Cell,
            owner: "OverseerDurableObject:abc".to_owned(),
            route: String::new(),
            node: "node-a".to_owned(),
            generation: 42,
            handle: 9,
            flags: 0,
        }
    }

    #[test]
    fn mint_and_verify_round_trip() {
        let key = key();
        let claims = claims();
        let token = key.mint(&claims).expect("mint");
        assert!(!token.is_empty());
        assert_eq!(key.verify(&token).expect("verify"), claims);
    }

    #[test]
    fn derived_key_is_deterministic_and_domain_separated() {
        let a = CapabilityKey::derive(b"fleet-secret").expect("derive");
        let b = CapabilityKey::derive(b"fleet-secret").expect("derive");
        let c = CapabilityKey::derive(b"other-secret").expect("derive");
        let token = a.mint(&claims()).expect("mint");
        assert_eq!(a.verify(&token).expect("verify"), claims());
        assert_eq!(b.verify(&token).expect("verify"), claims());
        assert_eq!(c.verify(&token), Err(CapabilityError::BadMac));
        assert_eq!(
            CapabilityKey::derive(b"").unwrap_err(),
            CapabilityError::EmptySecret
        );
    }

    #[test]
    fn tampering_is_rejected() {
        let key = key();
        let token = key.mint(&claims()).expect("mint");
        let mut bytes = URL_SAFE_NO_PAD.decode(token.as_bytes()).expect("decode");
        let payload_byte = bytes.len() - MAC_LEN - 1;
        bytes[payload_byte] ^= 0x01;
        let tampered = URL_SAFE_NO_PAD.encode(bytes);
        assert_eq!(key.verify(&tampered), Err(CapabilityError::BadMac));
    }

    #[test]
    fn wrong_key_is_rejected() {
        let token = key().mint(&claims()).expect("mint");
        let other = CapabilityKey::new([8_u8; 32]);
        assert_eq!(other.verify(&token), Err(CapabilityError::BadMac));
    }

    #[test]
    fn generation_is_heap_and_incarnation_specific() {
        assert_eq!(generation_for("heap-a"), generation_for("heap-a"));
        assert_ne!(generation_for("heap-a"), generation_for("heap-b"));
    }

    #[test]
    fn unknown_version_and_kind_are_rejected() {
        let key = key();
        let mut payload = encode_claims(&claims()).expect("encode");
        payload[0] = 99;
        let token = key.sign_payload(&payload).expect("sign");
        assert_eq!(key.verify(&token), Err(CapabilityError::UnknownVersion(99)));

        let mut payload = encode_claims(&claims()).expect("encode");
        payload[1] = 99;
        let token = key.sign_payload(&payload).expect("sign");
        assert_eq!(key.verify(&token), Err(CapabilityError::UnknownKind(99)));
    }

    #[test]
    fn malformed_tokens_are_rejected() {
        let key = key();
        assert_eq!(key.verify("not base64!"), Err(CapabilityError::Malformed));
        assert_eq!(key.verify("AAAA"), Err(CapabilityError::Malformed));
        let long = "a".repeat(MAX_TOKEN_LEN + 1);
        assert_eq!(key.verify(&long), Err(CapabilityError::TokenTooLong));
    }

    #[test]
    fn oversized_owner_and_node_are_rejected() {
        let key = key();
        let mut long_owner = claims();
        long_owner.owner = "x".repeat(MAX_OWNER_LEN + 1);
        assert_eq!(key.mint(&long_owner), Err(CapabilityError::OwnerTooLong));

        let mut long_node = claims();
        long_node.node = "n".repeat(MAX_NODE_LEN + 1);
        assert_eq!(key.mint(&long_node), Err(CapabilityError::NodeTooLong));
    }

    #[test]
    fn registry_tracks_references() {
        let mut registry = CapabilityRegistry::new();
        registry.register(claims(), 1).expect("register");
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.retain(9).expect("retain"), 2);
        assert!(!registry.release(9).expect("release"));
        assert_eq!(registry.get(9).expect("live").refs, 1);
        assert!(registry.release(9).expect("release"));
        assert!(registry.get(9).is_none());
        assert!(registry.is_empty());
        assert_eq!(registry.release(9), Err(CapabilityError::UnknownHandle));
    }

    #[test]
    fn registry_rejects_bad_registration() {
        let mut registry = CapabilityRegistry::new();
        assert_eq!(
            registry.register(claims(), 0),
            Err(CapabilityError::InvalidRefs)
        );
        registry.register(claims(), 1).expect("register");
        assert_eq!(
            registry.register(claims(), 1),
            Err(CapabilityError::DuplicateHandle)
        );
    }

    #[test]
    fn capability_flags_are_authenticated() {
        let key = key();
        let mut bridge = claims();
        bridge.handle = 11;
        // Reserved flags ride in the signed payload, so a bridge can label a
        // handle without letting a receiver relabel it.
        bridge.flags = FLAG_CALLABLE;
        let token = key.mint(&bridge).expect("mint");
        assert_eq!(key.verify(&token).expect("verify"), bridge);

        let mut payload = encode_claims(&bridge).expect("encode");
        payload[2] = 0;
        let relabelled = key.sign_payload(&payload).expect("sign");
        assert_eq!(
            key.verify(&relabelled).expect("verify").flags,
            0,
            "flags follow the signed payload"
        );
    }

    #[test]
    fn restore_flag_is_disjoint_from_the_callable_marker() {
        // `__cap_mint` sets bit 0 for a callable target; a persistent-stub
        // handle must not be mistaken for one (that collision dispatched
        // ordinary method stubs into the `[restore]` replay path).
        assert_eq!(FLAG_CALLABLE & FLAG_RESTORE, 0);
        assert_eq!(
            FLAG_FACET_RESTORE & (FLAG_CALLABLE | FLAG_RESTORE),
            0,
            "the facet marker sits beside, not on, the other flags"
        );
        let mut restored = claims();
        restored.handle = 12;
        restored.flags = FLAG_RESTORE;
        let token = key().mint(&restored).expect("mint");
        let verified = key().verify(&token).expect("verify");
        assert_eq!(verified.flags & FLAG_RESTORE, FLAG_RESTORE);
        assert_eq!(verified.flags & FLAG_CALLABLE, 0);
    }

    #[test]
    fn stream_bridge_reference_disposes_on_release() {
        let mut refs = CapabilityRefs::default();
        // `__liftStream` mints the bridge entry, which registers one exported
        // reference for the receiving side.
        assert_eq!(refs.retain("OverseerDurableObject:abc", 11), 1);
        // Disposing the received stream (explicitly or at context end) is the
        // last release, which is what tells the owner to cancel/abort its
        // source.
        assert_eq!(refs.release("OverseerDurableObject:abc", 11), Some(true));
        assert!(refs.is_empty());
        // Teardown can reach the same handle twice; a duplicate release is
        // inert rather than an underflow.
        assert_eq!(refs.release("OverseerDurableObject:abc", 11), None);
    }

    #[test]
    fn concurrent_release_disposes_exactly_once() {
        use std::sync::{Arc, Barrier, Mutex};
        let refs = Arc::new(Mutex::new(CapabilityRefs::default()));
        for _ in 0..8 {
            refs.lock().unwrap().retain("scope-a", 5);
        }
        let barrier = Arc::new(Barrier::new(8));
        let mut threads = Vec::new();
        for _ in 0..8 {
            let refs = Arc::clone(&refs);
            let barrier = Arc::clone(&barrier);
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                refs.lock().unwrap().release("scope-a", 5)
            }));
        }
        let results: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().expect("thread"))
            .collect();
        assert_eq!(
            results
                .iter()
                .filter(|result| **result == Some(true))
                .count(),
            1,
            "exactly one release owns the dispose"
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| **result == Some(false))
                .count(),
            7
        );
        assert!(refs.lock().unwrap().is_empty());
    }

    #[test]
    fn exported_refs_are_shared_by_owner_and_handle() {
        let mut refs = CapabilityRefs::default();
        assert_eq!(refs.retain("scope-a", 3), 1);
        assert_eq!(refs.retain("scope-a", 3), 2);
        assert_eq!(refs.retain("scope-b", 3), 1);
        assert_eq!(refs.count("scope-a", 3), Some(2));
        assert_eq!(refs.release("scope-a", 3), Some(false));
        assert_eq!(refs.release("scope-a", 3), Some(true));
        assert_eq!(refs.count("scope-a", 3), None);
        assert_eq!(refs.release("scope-a", 3), None);
        assert_eq!(refs.count("scope-b", 3), Some(1));
        assert_eq!(refs.len(), 1);
        assert!(!refs.is_empty());
    }
}
