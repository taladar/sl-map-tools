//! Axum extractor that resolves the real client IP address, honouring
//! `X-Forwarded-For` or `Forwarded` only when the direct peer is in the
//! configured `trusted_proxies` list.

use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;

use crate::config::ForwardedHeader;
use crate::state::AppState;

/// Resolved client IP address, ready for logging or storage in the
/// `sessions` row.
#[derive(Debug, Clone, Copy)]
pub struct ClientIp(pub IpAddr);

impl std::fmt::Display for ClientIp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extracting `ConnectInfo<SocketAddr>` requires that the server was
        // started with `.into_make_service_with_connect_info::<SocketAddr>()`.
        // If that's missing the extractor returns an error — fall back to an
        // unspecified address so request handling continues; client IP is
        // never used for auth decisions, only for audit logging.
        let peer = ConnectInfo::<SocketAddr>::from_request_parts(parts, state)
            .await
            .map_or(
                IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                |ConnectInfo(addr)| addr.ip(),
            );

        let cfg = &state.config;
        if matches!(cfg.forwarded_header, ForwardedHeader::None)
            || !is_trusted_proxy(&cfg.trusted_proxies, peer)
        {
            return Ok(Self(peer));
        }

        let resolved = match cfg.forwarded_header {
            ForwardedHeader::XForwardedFor => extract_x_forwarded_for(parts, &cfg.trusted_proxies),
            ForwardedHeader::Forwarded => extract_forwarded(parts, &cfg.trusted_proxies),
            ForwardedHeader::None => None,
        };

        Ok(Self(resolved.unwrap_or(peer)))
    }
}

/// True if `ip` matches any of the configured trusted-proxy CIDR ranges.
fn is_trusted_proxy(networks: &[ipnet::IpNet], ip: IpAddr) -> bool {
    networks.iter().any(|net| net.contains(&ip))
}

/// Walk the comma-separated `X-Forwarded-For` chain from the right, skipping
/// hops that are themselves in the trusted-proxies list, and return the
/// first non-trusted entry. That's the original client; everything to its
/// right is a proxy that's already in our trust list.
fn extract_x_forwarded_for(parts: &Parts, trusted: &[ipnet::IpNet]) -> Option<IpAddr> {
    let header = parts.headers.get_all("x-forwarded-for");
    // Join all header instances together; the RFC allows multiple headers
    // each with comma-separated entries.
    let combined: Vec<String> = header
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(','))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    let mut iter = combined.into_iter().rev();
    while let Some(entry) = iter.next() {
        match entry.parse::<IpAddr>() {
            Ok(ip) => {
                if is_trusted_proxy(trusted, ip) {
                    continue;
                }
                return Some(ip);
            }
            Err(err) => {
                tracing::warn!(
                    "ignoring un-parseable X-Forwarded-For entry {entry:?}: {err}; remaining chain ignored"
                );
                drop(iter);
                return None;
            }
        }
    }
    None
}

/// Walk the RFC 7239 `Forwarded` header from the right, picking the first
/// `for=` parameter whose value isn't itself a trusted proxy.
fn extract_forwarded(parts: &Parts, trusted: &[ipnet::IpNet]) -> Option<IpAddr> {
    let entries: Vec<String> = parts
        .headers
        .get_all("forwarded")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(','))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    let mut iter = entries.into_iter().rev();
    while let Some(entry) = iter.next() {
        let Some(for_value) = forwarded_for_param(&entry) else {
            continue;
        };
        match parse_forwarded_for_node(&for_value) {
            Some(ip) => {
                if is_trusted_proxy(trusted, ip) {
                    continue;
                }
                return Some(ip);
            }
            None => {
                tracing::warn!(
                    "ignoring un-parseable Forwarded for= node {for_value:?}; remaining chain ignored"
                );
                drop(iter);
                return None;
            }
        }
    }
    None
}

/// Find the `for=` parameter inside a single `Forwarded` entry. Parameters
/// are semicolon-separated, names are case-insensitive, values may be
/// quoted strings.
fn forwarded_for_param(entry: &str) -> Option<String> {
    for raw in entry.split(';') {
        let raw = raw.trim();
        let Some((name, value)) = raw.split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("for") {
            let v = value.trim();
            let stripped = v.strip_prefix('"').and_then(|s| s.strip_suffix('"'));
            return Some(stripped.unwrap_or(v).to_owned());
        }
    }
    None
}

/// Parse an RFC 7239 node identifier (IP, optionally bracketed IPv6,
/// optionally with `:port`) back into an [`IpAddr`].
fn parse_forwarded_for_node(node: &str) -> Option<IpAddr> {
    let trimmed = node.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
        return None;
    }
    // Bracketed IPv6: "[::1]" or "[::1]:1234".
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some((inside, _after)) = rest.split_once(']') {
            return inside.parse().ok();
        }
        return None;
    }
    // Try the whole thing as an IP first (covers bare IPv6 without brackets
    // and bare IPv4 without port).
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return Some(ip);
    }
    // IPv4 with port: "1.2.3.4:1234"
    if let Some((host, _port)) = trimmed.rsplit_once(':')
        && let Ok(ip) = host.parse::<IpAddr>()
    {
        return Some(ip);
    }
    None
}
