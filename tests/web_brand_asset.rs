// SPDX-License-Identifier: AGPL-3.0-or-later
use tokio::net::TcpListener;

#[tokio::test]
async fn web_serves_decorative_kanji_brand_mark() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let (endpoint, _task) = kaizen::web::start_with_listener(listener).await?;
    assert_index(&endpoint.listen).await?;
    assert_logo(&endpoint.listen).await
}

async fn assert_index(address: &str) -> anyhow::Result<()> {
    let body = reqwest::get(format!("http://{address}"))
        .await?
        .text()
        .await?;
    assert!(body.contains("src=\"/assets/kaizen-kanji.png\" alt=\"\" aria-hidden=\"true\""));
    Ok(())
}

async fn assert_logo(address: &str) -> anyhow::Result<()> {
    let response = reqwest::get(format!("http://{address}/assets/kaizen-kanji.png")).await?;
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(response.bytes().await?.starts_with(b"\x89PNG\r\n\x1a\n"));
    Ok(())
}
