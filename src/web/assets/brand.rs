// SPDX-License-Identifier: AGPL-3.0-or-later
//! Kaizen brand assets.

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;
use axum::{Router, routing::get};

const KANJI: &[u8] = include_bytes!("kaizen-kanji.png");

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new().route("/assets/kaizen-kanji.png", get(kanji))
}

pub async fn kanji() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "image/png"), (CACHE_CONTROL, "no-store")],
        KANJI,
    )
}
