use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Bytes;
use http_body::{Body, Frame, SizeHint};
use pin_project_lite::pin_project;
use tokio::sync::Notify;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedBody {
    pub bytes: Bytes,
    pub truncated: bool,
}

#[derive(Debug, Default)]
struct CaptureBuffer {
    bytes: Vec<u8>,
    truncated: bool,
}

#[derive(Debug)]
struct CaptureState {
    limit: usize,
    complete: AtomicBool,
    notify: Notify,
    buffer: Mutex<CaptureBuffer>,
}

impl CaptureState {
    fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            limit,
            complete: AtomicBool::new(false),
            notify: Notify::new(),
            buffer: Mutex::new(CaptureBuffer::default()),
        })
    }

    fn observe(&self, data: &Bytes) {
        let mut buffer = self.buffer.lock().expect("capture buffer poisoned");
        if buffer.bytes.len() >= self.limit {
            buffer.truncated = true;
            return;
        }

        let remaining = self.limit.saturating_sub(buffer.bytes.len());
        let take = remaining.min(data.len());
        buffer.bytes.extend_from_slice(&data[..take]);
        if take < data.len() {
            buffer.truncated = true;
        }
    }

    fn finish(&self) {
        if !self.complete.swap(true, Ordering::Release) {
            self.notify.notify_waiters();
        }
    }

    fn snapshot(&self) -> CapturedBody {
        let buffer = self.buffer.lock().expect("capture buffer poisoned");
        CapturedBody {
            bytes: Bytes::copy_from_slice(&buffer.bytes),
            truncated: buffer.truncated,
        }
    }
}

#[derive(Debug)]
struct CompletionGuard {
    state: Arc<CaptureState>,
    finished: bool,
}

impl CompletionGuard {
    fn new(state: Arc<CaptureState>) -> Self {
        Self {
            state,
            finished: false,
        }
    }

    fn finish(&mut self) {
        if !self.finished {
            self.finished = true;
            self.state.finish();
        }
    }
}

impl Drop for CompletionGuard {
    fn drop(&mut self) {
        self.finish();
    }
}

#[derive(Clone, Debug)]
pub struct CaptureHandle {
    state: Arc<CaptureState>,
}

impl CaptureHandle {
    pub async fn wait(self) -> CapturedBody {
        loop {
            if self.state.complete.load(Ordering::Acquire) {
                return self.state.snapshot();
            }
            self.state.notify.notified().await;
        }
    }
}

pin_project! {
    pub struct CaptureBody<B> {
        #[pin]
        inner: B,
        guard: CompletionGuard,
    }
}

impl<B> CaptureBody<B> {
    pub fn new(inner: B, limit: usize) -> (Self, CaptureHandle) {
        let state = CaptureState::new(limit);
        (
            Self {
                inner,
                guard: CompletionGuard::new(state.clone()),
            },
            CaptureHandle { state },
        )
    }
}

impl<B> Body for CaptureBody<B>
where
    B: Body<Data = Bytes>,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();

        match this.inner.poll_frame(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(frame))) => match frame.into_data() {
                Ok(data) => {
                    this.guard.state.observe(&data);
                    Poll::Ready(Some(Ok(Frame::data(data))))
                }
                Err(frame) => Poll::Ready(Some(Ok(frame))),
            },
            Poll::Ready(Some(Err(error))) => {
                this.guard.finish();
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                this.guard.finish();
                Poll::Ready(None)
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use axum::body::{Body as AxumBody, Bytes};
    use futures::stream;
    use http_body::Frame;
    use http_body_util::{BodyExt, StreamBody};

    use super::{CaptureBody, CapturedBody};

    async fn collect_capture<B>(body: B) -> CapturedBody
    where
        B: http_body::Body<Data = Bytes> + Unpin,
        B::Error: std::fmt::Debug,
    {
        let (wrapped, handle) = CaptureBody::new(body, 4);
        let _ = wrapped.collect().await.unwrap();
        handle.wait().await
    }

    #[tokio::test]
    async fn capture_retains_prefix_only() {
        let body = AxumBody::from("abcdef");
        let captured = collect_capture(body).await;
        assert_eq!(captured.bytes, Bytes::from_static(b"abcd"));
        assert!(captured.truncated);
    }

    #[tokio::test]
    async fn capture_marks_truncation_across_frames() {
        let stream = stream::iter(vec![
            Ok::<_, std::convert::Infallible>(Frame::data(Bytes::from_static(b"ab"))),
            Ok::<_, std::convert::Infallible>(Frame::data(Bytes::from_static(b"cdef"))),
        ]);
        let body = StreamBody::new(stream);
        let captured = collect_capture(body).await;
        assert_eq!(captured.bytes, Bytes::from_static(b"abcd"));
        assert!(captured.truncated);
    }

    #[tokio::test]
    async fn capture_completes_on_drop() {
        let body = AxumBody::from("abcdef");
        let (wrapped, handle) = CaptureBody::new(body, 10);
        drop(wrapped);
        let captured = handle.wait().await;
        assert_eq!(captured.bytes, Bytes::new());
        assert!(!captured.truncated);
    }
}
