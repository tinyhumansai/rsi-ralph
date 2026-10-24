//! A minimal single-future executor for synchronous callers.
//!
//! The improvement loops are synchronous, but [`crate::ModelPort`] returns a
//! future. [`block_on`] drives one future to completion on the calling thread,
//! parking between polls, so a loop can call a model without owning an async
//! runtime. The port must therefore not require a specific runtime;
//! [`crate::OpenRouterModel`] does not.
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    thread::{self, Thread},
};

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Drives `future` to completion on the current thread and returns its output.
///
/// # Examples
///
/// ```
/// assert_eq!(rsi_ralph_model::block_on(async { 6 * 7 }), 42);
/// ```
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::park(),
        }
    }
}

#[cfg(test)]
mod test;
