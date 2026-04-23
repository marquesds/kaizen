// SPDX-License-Identifier: AGPL-3.0-or-later
//! Forward allowed request headers; filter hop-by-hop on responses.

use axum::http::HeaderMap;
use axum::http::HeaderName;
use axum::http::header;

const HOP: [&str; 6] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "upgrade",
];

pub fn is_hop(n: &HeaderName) -> bool {
    let l = n.as_str().to_ascii_lowercase();
    HOP.iter().any(|&x| x == l) || l == "transfer-encoding" || l == "proxy-connection"
}

/// Headers clients send that we try to pass through.
pub fn copy_req_headers(in_h: &HeaderMap, out: &mut HeaderMap) {
    for (k, v) in in_h.iter() {
        let s = k.as_str().to_ascii_lowercase();
        if s == "host" || s == "content-length" || s == "x-kaizen-session" {
            continue;
        }
        if is_hop(k) {
            continue;
        }
        out.insert(k, v.clone());
    }
    out.remove(header::HOST);
}
