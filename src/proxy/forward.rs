// SPDX-License-Identifier: AGPL-3.0-or-later
//! One HTTP round-trip: optional JSON minify, upstream `reqwest`, one SQLite row.

use crate::proxy::http_ext;
use crate::proxy::record::{self, RecordArgs};
use crate::proxy::sse::find_usage_in_body;
use crate::proxy::state::ProxyState;
use crate::proxy::transform;
use axum::body::Body;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use std::str::from_utf8;
use std::sync::Arc;
use uuid::Uuid;

/// Core: build URL, transform JSON, send, record, return client response.
pub async fn run_forward_inner(
    st: &Arc<ProxyState>,
    method: axum::http::Method,
    path: &str,
    query: &str,
    headers: &axum::http::HeaderMap,
    body: &axum::body::Bytes,
) -> Result<Response, anyhow::Error> {
    use axum::http::Method;
    let session_id = session_id_from(headers);
    let upstream = st.options.upstream.trim_end_matches('/');
    let url = if path.is_empty() {
        upstream.to_string()
    } else {
        format!("{upstream}/{path}")
    };
    let mut full = url;
    if !query.is_empty() {
        full.push('?');
        full.push_str(query);
    }
    let is_json = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.to_lowercase().contains("application/json"));
    let (out_body, model_opt) =
        if is_json && matches!(method, Method::POST | Method::PUT | Method::PATCH) {
            let raw = body.as_ref();
            let json_val: Option<serde_json::Value> = serde_json::from_slice(raw).ok();
            let model = json_val.as_ref().and_then(transform::try_model);
            let processed = if json_val.is_some() {
                transform::process_request_bytes(
                    raw,
                    st.options.minify_json,
                    &st.options.context_policy,
                )
            } else {
                Ok(body.to_vec())
            };
            let b = match processed {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(error = %e, "proxy body transform — forwarding raw");
                    body.to_vec()
                }
            };
            (b, model)
        } else {
            (body.to_vec(), None)
        };
    let mut rheaders = axum::http::HeaderMap::new();
    http_ext::copy_req_headers(headers, &mut rheaders);
    let ureq: reqwest::Url = full
        .parse()
        .map_err(|e| anyhow::anyhow!(r#"bad upstream url "{full}": {e}"#))?;
    let sent = st
        .client
        .request(method.clone(), ureq)
        .headers(rheaders)
        .body(out_body);
    let reqwest_resp = match sent.send().await {
        Ok(x) => x,
        Err(e) => {
            record_spawn(
                st,
                RecordArgs {
                    session_id: session_id.clone(),
                    model: model_opt.clone(),
                    path: path.to_string(),
                    method: method.to_string(),
                    status: 0,
                    request_id: None,
                    tokens_in: None,
                    tokens_out: None,
                    reasoning_tokens: None,
                    upstream_error: Some(format!("{e}")),
                },
            )
            .await;
            return Err(e.into());
        }
    };
    let status = reqwest_resp.status();
    let res_headers = reqwest_resp.headers().clone();
    let ctype = res_headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(std::string::ToString::to_string);
    let ubytes = match reqwest_resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            record_spawn(
                st,
                RecordArgs {
                    session_id: session_id.clone(),
                    model: model_opt.clone(),
                    path: path.to_string(),
                    method: method.to_string(),
                    status: status.as_u16(),
                    request_id: None,
                    tokens_in: None,
                    tokens_out: None,
                    reasoning_tokens: None,
                    upstream_error: Some(format!("read body: {e}")),
                },
            )
            .await;
            return Err(e.into());
        }
    };
    if ubytes.len() as u64 > st.options.max_response_bytes {
        record_spawn(
            st,
            RecordArgs {
                session_id: session_id.clone(),
                model: model_opt.clone(),
                path: path.to_string(),
                method: method.to_string(),
                status: status.as_u16(),
                request_id: None,
                tokens_in: None,
                tokens_out: None,
                reasoning_tokens: None,
                upstream_error: Some("upstream body exceeds `proxy.max_response_body_mb`".into()),
            },
        )
        .await;
        return Ok((
            StatusCode::BAD_GATEWAY,
            "kaizen proxy: response over `max_response_body_mb` (raise in config)",
        )
            .into_response());
    }
    let is_sse = ctype
        .as_deref()
        .is_some_and(|c| c.to_lowercase().contains("text/event-stream"));
    let (ti, to, tr) = find_usage_in_body(ubytes.as_ref(), is_sse);
    let rid = res_headers
        .get("x-request-id")
        .or_else(|| res_headers.get("request-id"))
        .and_then(|v| v.to_str().ok().map(String::from));
    record_spawn(
        st,
        RecordArgs {
            session_id: session_id.clone(),
            model: model_opt,
            path: path.to_string(),
            method: method.to_string(),
            status: status.as_u16(),
            request_id: rid,
            tokens_in: ti,
            tokens_out: to,
            reasoning_tokens: tr,
            upstream_error: if status.is_success() {
                None
            } else {
                Some(truncate_err_msg(ubytes.as_ref(), 400))
            },
        },
    )
    .await;
    let mut h2 = axum::http::HeaderMap::new();
    for (k, v) in res_headers.iter() {
        if http_ext::is_hop(k) {
            continue;
        }
        h2.append(k, v.clone());
    }
    let mut b = axum::response::Response::builder().status(status.as_u16());
    for (k, v) in h2 {
        if let Some(n) = k {
            b = b.header(n, v);
        }
    }
    Ok(b.body(Body::from(ubytes))?)
}

fn session_id_from(headers: &axum::http::HeaderMap) -> String {
    for (k, v) in headers.iter() {
        if k.as_str().eq_ignore_ascii_case("x-kaizen-session")
            && let Ok(s) = v.to_str()
            && !s.is_empty()
        {
            return s.to_string();
        }
    }
    format!("proxy-{}", Uuid::now_v7())
}

fn truncate_err_msg(b: &[u8], n: usize) -> String {
    let t = from_utf8(b).unwrap_or("<non-utf8 body>");
    let s: String = t.chars().take(n).collect();
    if t.chars().count() > n {
        format!("upstream error body (first {n} chars): {s}…")
    } else {
        format!("upstream error body: {s}")
    }
}

async fn record_spawn(st: &Arc<ProxyState>, a: RecordArgs) {
    let path = st.store_path.clone();
    let cfg = st.config.clone();
    let w = st.workspace.clone();
    match tokio::task::spawn_blocking(move || record::record_forward_outcome(&path, &cfg, &w, &a))
        .await
    {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!(%e, "record_forward_outcome"),
        Err(e) => tracing::warn!(?e, "record task join"),
    }
}
