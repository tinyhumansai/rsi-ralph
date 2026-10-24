//! Unit tests for the single-future executor.

#![expect(clippy::unwrap_used, reason = "assert deterministic test fixtures")]

use super::*;
use std::{pin::Pin, sync::Mutex};

/// Pending until polled `remaining` more times, waking itself each time.
struct Countdown {
    remaining: u32,
}

impl Future for Countdown {
    type Output = &'static str;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.remaining == 0 {
            return Poll::Ready("done");
        }
        self.remaining -= 1;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

#[test]
fn returns_a_ready_value() {
    assert_eq!(block_on(async { 7 }), 7);
}

#[test]
fn polls_a_pending_future_until_it_is_ready() {
    assert_eq!(block_on(Countdown { remaining: 3 }), "done");
}

#[test]
fn parks_until_another_thread_wakes_it() {
    /// Pending until a spawned thread fills the slot and wakes the task once.
    struct Later(Arc<Mutex<Option<u8>>>, bool);
    impl Future for Later {
        type Output = u8;
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u8> {
            if let Some(v) = *self.0.lock().unwrap() {
                return Poll::Ready(v);
            }
            if !self.1 {
                self.1 = true;
                let slot = self.0.clone();
                let waker = cx.waker().clone();
                thread::spawn(move || {
                    *slot.lock().unwrap() = Some(4);
                    waker.wake();
                });
            }
            Poll::Pending
        }
    }
    assert_eq!(block_on(Later(Arc::new(Mutex::new(None)), false)), 4);
}
