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
static OWN_ADVERTISE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
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

/// Installs the address peers dial for this process. A stateless capability
/// carries it so its owner can be reached from another node without a
/// registry read on the call path.
pub fn set_advertise(advertise: &str) {
    let _ = OWN_ADVERTISE.set(advertise.to_owned());
}

/// The address peers dial for this process (empty in tests).
pub fn own_advertise() -> &'static str {
    OWN_ADVERTISE.get().map(String::as_str).unwrap_or("")
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
const TOKEN_VERSION: u8 = 3;
/// Version 2 predates the owner address. A rolling deploy can still meet one,
/// so it parses (with no address) and only loses the cross-node stateless
/// call, which fails typed as before.
const LEGACY_VERSION_NO_ADDR: u8 = 2;
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
const MAX_ADDR_LEN: usize = 256;
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
    /// Address peers dial for that node, so a stateless capability can be
    /// called from another node without a registry read. Empty for cell
    /// handles (their route names the owner) and for legacy tokens.
    pub addr: String,
    /// Owner incarnation. A token from an older incarnation is stale.
    pub generation: u64,
    /// Handle id within the owner.
    pub handle: u64,
    /// Reserved for caller-visible flags (dispose-only, durable, ...).
    pub flags: u8,
}

impl CapabilityClaims {
    /// May this capability be written down as a token and revived later?
    ///
    /// The storage rule refuses a transient stub because an entry id means
    /// nothing after a restart. The token is different: it is signed by the
    /// fleet key and names the owner's node, the owner's isolate incarnation and
    /// the address peers dial, so a revived handle can be re-verified at call
    /// time and refused typed once that owner is gone. Only a handle carrying
    /// that whole fence may be persisted; anything weaker would resolve to a
    /// reused entry after a restart instead of failing, which is exactly the
    /// aliasing the token model exists to prevent.
    pub fn revival_fence(&self) -> Result<(), RevivalFenceError> {
        if self.flags & FLAG_RESTORE != 0 {
            return Err(RevivalFenceError::DurableHandle);
        }
        if self.kind != CapabilityKind::Stateless {
            return Err(RevivalFenceError::CellHandle);
        }
        if self.node.is_empty() {
            return Err(RevivalFenceError::NoNode);
        }
        if self.generation == 0 {
            return Err(RevivalFenceError::NoGeneration);
        }
        if self.addr.is_empty() {
            return Err(RevivalFenceError::NoAddress);
        }
        Ok(())
    }

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
            addr: String::new(),
            generation,
            handle,
            flags: 0,
        }
    }
}

/// Why a capability may not be persisted as a token and revived later.
///
/// Each variant is a different reason a restored owner would have nothing to
/// verify against, so none of them is a policy choice the application can
/// override: an unfenced handle stays refused.
#[derive(Debug, PartialEq, Eq)]
pub enum RevivalFenceError {
    /// The token itself is unusable (encoding, MAC, version).
    Token(CapabilityError),
    /// A version 1 transient token predates the node/incarnation fence.
    Legacy,
    /// A durable (`ctx.restore`) handle is already persisted by its row; the
    /// stub, not a copied token, is the thing to store.
    DurableHandle,
    /// A cell-owned handle lives in the isolate serving that cell, so it is
    /// gone by the time a revived copy could be used.
    CellHandle,
    /// The token names no node.
    NoNode,
    /// The token names no isolate incarnation.
    NoGeneration,
    /// The token carries no owner address, so it could not be called from the
    /// node that revived it after the object moved.
    NoAddress,
}

impl fmt::Display for RevivalFenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Token(error) => write!(f, "{error}"),
            Self::Legacy => write!(
                f,
                "legacy transient capability has no cross-node fence; drain the old node and re-acquire"
            ),
            Self::DurableHandle => write!(
                f,
                "this handle is already durable (ctx.restore): store the stub itself"
            ),
            Self::CellHandle => write!(
                f,
                "only a stateless (session) capability can be persisted: a cell handle belongs to the isolate that serves the cell"
            ),
            Self::NoNode => write!(
                f,
                "this capability names no owning node, so a restart could not tell it from a reused handle"
            ),
            Self::NoGeneration => write!(
                f,
                "this capability names no owner isolate incarnation, so a restart could not tell it from a reused handle"
            ),
            Self::NoAddress => write!(
                f,
                "this capability carries no owner address, so it could not be called after its object moves to another node"
            ),
        }
    }
}

impl std::error::Error for RevivalFenceError {}

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
        let payload = self.verified_payload(token)?;
        parse_claims(&payload)
    }

    /// Verifies a token across a rolling upgrade. Version 1 tokens predate the
    /// owner node, incarnation and facet route, so they are only safe for
    /// durable handles: a transient handle needs the node/incarnation fence.
    /// Returns `(claims, legacy)`.
    pub fn verify_compat(&self, token: &str) -> Result<(CapabilityClaims, bool), CapabilityError> {
        let payload = self.verified_payload(token)?;
        match payload.first().copied() {
            Some(TOKEN_VERSION) | Some(LEGACY_VERSION_NO_ADDR) => {
                parse_claims(&payload).map(|claims| (claims, false))
            }
            Some(1) => parse_claims_v1(&payload).map(|claims| (claims, true)),
            Some(version) => Err(CapabilityError::UnknownVersion(version)),
            None => Err(CapabilityError::Malformed),
        }
    }

    /// Verifies a token a caller wants to persist and revive later.
    ///
    /// This is the authoritative answer to "may I store this token?", and it
    /// deliberately re-applies the rolling-upgrade rule: a version 1 transient
    /// token has no fence, so it is refused here for the same reason the call
    /// path refuses it. It does **not** check the fence against this process:
    /// the token names another node (that is the point), and whether that owner
    /// is still there is decided by the call, which answers `owner-gone`,
    /// `owner-moved` or `stale-generation`.
    pub fn verify_revivable(&self, token: &str) -> Result<CapabilityClaims, RevivalFenceError> {
        let (claims, legacy) = self
            .verify_compat(token)
            .map_err(RevivalFenceError::Token)?;
        if !legacy_is_safe(legacy, claims.flags) {
            return Err(RevivalFenceError::Legacy);
        }
        claims.revival_fence()?;
        Ok(claims)
    }

    fn verified_payload(&self, token: &str) -> Result<Vec<u8>, CapabilityError> {
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
        Ok(payload.to_vec())
    }
}

/// A version 1 token has no node, incarnation or route, so only a durable
/// handle (whose owner row is re-resolved by scope) may use it.
pub fn legacy_is_safe(legacy: bool, flags: u8) -> bool {
    !legacy || flags & FLAG_RESTORE != 0
}

fn encode_claims(claims: &CapabilityClaims) -> Result<Vec<u8>, CapabilityError> {
    let owner = claims.owner.as_bytes();
    let route = claims.route.as_bytes();
    let node = claims.node.as_bytes();
    let addr = claims.addr.as_bytes();
    if owner.len() > MAX_OWNER_LEN {
        return Err(CapabilityError::OwnerTooLong);
    }
    if node.len() > MAX_NODE_LEN {
        return Err(CapabilityError::NodeTooLong);
    }
    if addr.len() > MAX_ADDR_LEN {
        return Err(CapabilityError::NodeTooLong);
    }
    let owner_len = u16::try_from(owner.len()).map_err(|_| CapabilityError::OwnerTooLong)?;
    let node_len = u16::try_from(node.len()).map_err(|_| CapabilityError::NodeTooLong)?;
    let route_len = u16::try_from(route.len()).map_err(|_| CapabilityError::OwnerTooLong)?;
    let addr_len = u16::try_from(addr.len()).map_err(|_| CapabilityError::NodeTooLong)?;

    let mut payload = Vec::with_capacity(
        1 + 1 + 1 + 8 + 8 + 2 + owner.len() + 2 + node.len() + 2 + route.len() + 2 + addr.len(),
    );
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
    payload.extend_from_slice(&addr_len.to_le_bytes());
    payload.extend_from_slice(addr);
    Ok(payload)
}

fn parse_claims(payload: &[u8]) -> Result<CapabilityClaims, CapabilityError> {
    let mut cursor = 0_usize;
    let version = take_u8(payload, &mut cursor)?;
    if version != TOKEN_VERSION && version != LEGACY_VERSION_NO_ADDR {
        return Err(CapabilityError::UnknownVersion(version));
    }
    let kind = CapabilityKind::from_u8(take_u8(payload, &mut cursor)?)?;
    let flags = take_u8(payload, &mut cursor)?;
    let generation = take_u64(payload, &mut cursor)?;
    let handle = take_u64(payload, &mut cursor)?;
    let owner = take_string(payload, &mut cursor, MAX_OWNER_LEN)?;
    let node = take_string(payload, &mut cursor, MAX_NODE_LEN)?;
    let route = take_string(payload, &mut cursor, MAX_OWNER_LEN)?;
    let addr = if version >= TOKEN_VERSION {
        take_string(payload, &mut cursor, MAX_ADDR_LEN)?
    } else {
        String::new()
    };
    if cursor != payload.len() {
        return Err(CapabilityError::Malformed);
    }
    Ok(CapabilityClaims {
        kind,
        owner,
        route,
        node,
        addr,
        generation,
        handle,
        flags,
    })
}

fn parse_claims_v1(payload: &[u8]) -> Result<CapabilityClaims, CapabilityError> {
    let mut cursor = 0_usize;
    if take_u8(payload, &mut cursor)? != 1 {
        return Err(CapabilityError::Malformed);
    }
    let kind = CapabilityKind::from_u8(take_u8(payload, &mut cursor)?)?;
    let flags = take_u8(payload, &mut cursor)?;
    let generation = take_u64(payload, &mut cursor)?;
    let handle = take_u64(payload, &mut cursor)?;
    let owner = take_string(payload, &mut cursor, MAX_OWNER_LEN)?;
    let node = take_string(payload, &mut cursor, MAX_NODE_LEN)?;
    if cursor != payload.len() {
        return Err(CapabilityError::Malformed);
    }
    Ok(CapabilityClaims {
        kind,
        owner,
        route: String::new(),
        node,
        addr: String::new(),
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
            addr: "10.0.0.1:9999".to_owned(),
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

    fn v1_payload(claims: &CapabilityClaims) -> Vec<u8> {
        let owner = claims.owner.as_bytes();
        let node = claims.node.as_bytes();
        let mut payload = Vec::new();
        payload.push(1);
        payload.push(claims.kind.as_u8());
        payload.push(claims.flags);
        payload.extend_from_slice(&claims.generation.to_le_bytes());
        payload.extend_from_slice(&claims.handle.to_le_bytes());
        payload.extend_from_slice(&(owner.len() as u16).to_le_bytes());
        payload.extend_from_slice(owner);
        payload.extend_from_slice(&(node.len() as u16).to_le_bytes());
        payload.extend_from_slice(node);
        payload
    }

    #[test]
    fn legacy_tokens_are_durable_only() {
        let key = key();
        let mut durable = claims();
        durable.flags = FLAG_RESTORE;
        let token = key.sign_payload(&v1_payload(&durable)).expect("sign");
        assert_eq!(key.verify(&token), Err(CapabilityError::UnknownVersion(1)));
        let (parsed, legacy) = key.verify_compat(&token).expect("compat");
        assert!(legacy);
        assert_eq!(parsed.route, String::new());
        assert!(legacy_is_safe(legacy, parsed.flags));

        let mut transient = claims();
        transient.flags = 0;
        let token = key.sign_payload(&v1_payload(&transient)).expect("sign");
        let (parsed, legacy) = key.verify_compat(&token).expect("compat");
        assert!(!legacy_is_safe(legacy, parsed.flags));

        let token = key.mint(&claims()).expect("mint");
        let (_, legacy) = key.verify_compat(&token).expect("compat");
        assert!(!legacy);
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

    fn session_claims() -> CapabilityClaims {
        CapabilityClaims {
            kind: CapabilityKind::Stateless,
            owner: "heap-7".to_owned(),
            route: String::new(),
            node: "node-a".to_owned(),
            addr: "10.0.0.1:9999".to_owned(),
            generation: 0x1234_5678,
            handle: 3,
            flags: FLAG_CALLABLE,
        }
    }

    #[test]
    fn a_fenced_session_capability_may_be_persisted() {
        let key = key();
        let token = key.mint(&session_claims()).expect("mint");
        let claims = key.verify_revivable(&token).expect("revivable");
        assert_eq!(claims.kind, CapabilityKind::Stateless);
        assert_eq!(claims.node, "node-a");
    }

    /// The check is the token's shape, not this process's identity: the owner
    /// lives on another node, and whether it still exists is decided at call
    /// time so a dead owner answers typed instead of being pre-judged here.
    #[test]
    fn revival_accepts_a_foreign_owner_and_defers_liveness_to_the_call() {
        let key = key();
        let token = key.mint(&session_claims()).expect("mint");
        assert_ne!(own_node(), "node-a");
        assert!(key.verify_revivable(&token).is_ok());
    }

    #[test]
    fn unfenced_handles_stay_refused() {
        let key = key();

        let mut cell = session_claims();
        cell.kind = CapabilityKind::Cell;
        cell.owner = "Chat:abc".to_owned();
        assert_eq!(
            key.verify_revivable(&key.mint(&cell).expect("mint")),
            Err(RevivalFenceError::CellHandle)
        );

        let mut no_node = session_claims();
        no_node.node = String::new();
        assert_eq!(
            key.verify_revivable(&key.mint(&no_node).expect("mint")),
            Err(RevivalFenceError::NoNode)
        );

        let mut no_generation = session_claims();
        no_generation.generation = 0;
        assert_eq!(
            key.verify_revivable(&key.mint(&no_generation).expect("mint")),
            Err(RevivalFenceError::NoGeneration)
        );

        let mut no_address = session_claims();
        no_address.addr = String::new();
        assert_eq!(
            key.verify_revivable(&key.mint(&no_address).expect("mint")),
            Err(RevivalFenceError::NoAddress)
        );

        // A durable handle is already persisted by its row.
        let mut durable = session_claims();
        durable.flags = FLAG_RESTORE;
        assert_eq!(
            key.verify_revivable(&key.mint(&durable).expect("mint")),
            Err(RevivalFenceError::DurableHandle)
        );

        // A token from another key never revives.
        let other = CapabilityKey::new([9_u8; 32]);
        let token = other.mint(&session_claims()).expect("mint");
        assert_eq!(
            key.verify_revivable(&token),
            Err(RevivalFenceError::Token(CapabilityError::BadMac))
        );
        assert_eq!(
            key.verify_revivable("not base64!"),
            Err(RevivalFenceError::Token(CapabilityError::Malformed))
        );
    }

    /// A version 1 transient token predates the node/incarnation fence, so the
    /// compat window does not make it persistable: reviving it could alias a
    /// reused handle after a restart.
    #[test]
    fn a_legacy_transient_token_is_not_revivable() {
        let key = key();
        let mut transient = session_claims();
        transient.flags = 0;
        let token = key.sign_payload(&v1_payload(&transient)).expect("sign");
        assert_eq!(key.verify_revivable(&token), Err(RevivalFenceError::Legacy));

        // The durable v1 path stays open, but a durable row is persisted by the
        // row itself, so it still is not something to copy a token for.
        let mut durable = session_claims();
        durable.flags = FLAG_RESTORE;
        let token = key.sign_payload(&v1_payload(&durable)).expect("sign");
        assert_eq!(
            key.verify_revivable(&token),
            Err(RevivalFenceError::DurableHandle)
        );
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
