// SPDX-License-Identifier: AGPL-3.0-or-later

use kaizen::ipc::{MAX_FRAME_SIZE, read_frame, write_frame};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, ReadBuf};

struct HeaderOnlyReader(Option<[u8; 4]>);

impl AsyncRead for HeaderOnlyReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let header = self.0.take().expect("oversized frame read payload");
        buf.put_slice(&header);
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn oversized_frame_is_rejected_before_payload_read() {
    let length = MAX_FRAME_SIZE as u32 + 1;
    let mut reader = HeaderOnlyReader(Some(length.to_be_bytes()));
    let error = read_frame::<serde_json::Value, _>(&mut reader)
        .await
        .expect_err("oversized frame must fail");

    assert_eq!(
        error.to_string(),
        format!("IPC frame length {length} exceeds maximum {MAX_FRAME_SIZE} bytes")
    );
}

#[tokio::test]
async fn valid_frame_round_trips() {
    let expected = serde_json::json!({"type": "status"});
    let (mut writer, mut reader) = tokio::io::duplex(128);
    write_frame(&mut writer, &expected).await.unwrap();

    let actual = read_frame::<serde_json::Value, _>(&mut reader)
        .await
        .unwrap();
    assert_eq!(actual, expected);
}
