// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! Scrape authorization for the operator `/metrics` endpoint, reified sans-IO.
//!
//! `/metrics` lives on the internal listener, so its default protection is the
//! network boundary (loopback or a private overlay). An operator who scrapes
//! across a boundary a bearer token does not cover can require
//! `Authorization: Bearer <token>`; this module owns that decision as a pure
//! predicate so the exposition route and the tests agree on it.
//!
//! The comparison is constant time. A token check that returns on the first
//! mismatched byte leaks a prefix to a caller who can time the response, which
//! is the same leak a partial signature check would have.

/// The verdict for one scrape request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub enum MetricsAccess {
    /// Serve the exposition.
    Allow,
    /// Refuse with 401 and a `WWW-Authenticate` challenge.
    Unauthorized,
}

/// Compare two byte strings without an early return.
///
/// The length check leaks the configured token's length, which an attacker who
/// can count bytes already knows from a refused request; the bytes themselves
/// compare in fixed time for equal lengths.
fn ct_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}

/// May this request read the exposition?
///
/// `required` is the configured token, or `None` when the operator relies on
/// the network boundary alone. A configured token that is empty or blank is
/// treated as absent: an accidental empty value must not silently lock every
/// scraper out.
pub fn metrics_access(required: Option<&str>, authorization: Option<&str>) -> MetricsAccess {
    let Some(required) = required.map(str::trim).filter(|token| !token.is_empty()) else {
        return MetricsAccess::Allow;
    };
    let Some(value) = authorization else {
        return MetricsAccess::Unauthorized;
    };
    // RFC 9110 makes the scheme case-insensitive, so `bearer` and `BEARER`
    // must authenticate exactly like `Bearer`.
    let Some((scheme, token)) = value.split_once(' ') else {
        return MetricsAccess::Unauthorized;
    };
    if !scheme.eq_ignore_ascii_case("bearer") {
        return MetricsAccess::Unauthorized;
    }
    if ct_eq(token.trim().as_bytes(), required.as_bytes()) {
        MetricsAccess::Allow
    } else {
        MetricsAccess::Unauthorized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unconfigured_token_leaves_the_endpoint_open() {
        assert_eq!(metrics_access(None, None), MetricsAccess::Allow);
        assert_eq!(metrics_access(Some(""), None), MetricsAccess::Allow);
        assert_eq!(metrics_access(Some("   "), None), MetricsAccess::Allow);
    }

    #[test]
    fn a_configured_token_requires_a_bearer_header() {
        assert_eq!(
            metrics_access(Some("scrape-secret"), None),
            MetricsAccess::Unauthorized
        );
        assert_eq!(
            metrics_access(Some("scrape-secret"), Some("scrape-secret")),
            MetricsAccess::Unauthorized
        );
        assert_eq!(
            metrics_access(Some("scrape-secret"), Some("Basic scrape-secret")),
            MetricsAccess::Unauthorized
        );
    }

    #[test]
    fn a_matching_bearer_token_is_allowed_case_insensitively() {
        for header in [
            "Bearer scrape-secret",
            "bearer scrape-secret",
            "BEARER scrape-secret",
            "Bearer  scrape-secret ",
        ] {
            assert_eq!(
                metrics_access(Some("scrape-secret"), Some(header)),
                MetricsAccess::Allow,
                "header {header:?} should authenticate"
            );
        }
    }

    #[test]
    fn a_different_token_is_refused_and_a_prefix_does_not_authenticate() {
        for header in [
            "Bearer wrong",
            "Bearer scrape-secre",
            "Bearer scrapp-secret",
            "Bearer scrape-secret-extra",
            "Bearer ",
        ] {
            assert_eq!(
                metrics_access(Some("scrape-secret"), Some(header)),
                MetricsAccess::Unauthorized,
                "header {header:?} must not authenticate"
            );
        }
    }
}
