//! Adds a new Stream combinator able to call a single function once the Stream ends or is canceled
//! 
//! Please see [crate::StreamWithFinalizationCallbacks] for a version that does indeed distinguish between
//! completion and cancellation.

use std::{
    pin::Pin,
    task::{Context, Poll},
};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use futures::Stream;

/// A Stream wrapper that calls a closure once the Stream either ends or is cancelled:
pub struct StreamWithFinalizationCallback<S, FinalizationFn, FnFut>
where
    S: Stream,
    FinalizationFn: FnOnce() -> FnFut,
    FnFut: Future<Output = ()> + Send + 'static,
{
    inner: S,
    finalization_fn: Option<FinalizationFn>,
    /// Needed to avoid double-firing when the stream ends gracefully in one thread and
    /// is immediately dropped by another
    finalized: AtomicBool,
}

impl<S, FinalizationFn, FnFut> StreamWithFinalizationCallback<S, FinalizationFn, FnFut>
where
    S: Stream,
    FinalizationFn: FnOnce() -> FnFut,
    FnFut: Future<Output = ()> + Send + 'static,
{
    /// Construct a new wrapper that calls `finalization_fn` once when either:
    ///  - `inner.poll_next()` returns `Ready(None)`, or
    ///  - the wrapper is dropped before seeing `None`.
    pub fn new(inner: S, finalization_fn: FinalizationFn) -> Self {
        StreamWithFinalizationCallback {
            inner,
            finalization_fn: Some(finalization_fn),
            finalized: AtomicBool::new(false),
        }
    }
}

impl<S, FinalizationFn, FnFut, T> Stream for StreamWithFinalizationCallback<S, FinalizationFn, FnFut>
where
    S: Stream<Item = T> + Unpin,
    FinalizationFn: FnOnce() -> FnFut,
    FnFut: Future<Output = ()> + Send + 'static,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // SAFETY:
        //   - We only call `get_unchecked_mut()` because we know:
        //     1) `StreamWithCallbacks<…>` is structurally pinned (it won’t be moved after pinned),
        //     2) We never move `inner` or the callback fields out of that pinned memory except by taking them (which is OK),
        //     3) `inner: S` is `Unpin`, so it’s safe to create a `Pin<&mut S>` from `&mut inner`.
        //
        // In other words, after calling `get_unchecked_mut()`, we are free to mutate
        // the fields through `this`, and then re-pin `inner` via `Pin::new(&mut this.inner)`.
        let this: &mut Self = unsafe { self.get_unchecked_mut() };

        match Pin::new(&mut this.inner).poll_next(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some(item)),

            Poll::Ready(None) => {
                // The inner stream is done. If we have not yet called `finalization_fn`, do so now.
                let finalized = this.finalized.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |_| Some(true)).is_ok();
                if !finalized {
                    this.finalization_fn.take()
                        .map(|finalization_fn| tokio::runtime::Handle::current().spawn(finalization_fn()));
                }
                Poll::Ready(None)
            }

            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S, FinalizationFn, FnFut> Drop for StreamWithFinalizationCallback<S, FinalizationFn, FnFut>
where
    S: Stream,
    FinalizationFn: FnOnce() -> FnFut,
    FnFut: Future<Output = ()> + Send + 'static,
{
    fn drop(&mut self) {
        // If we never reached the “finished” state, that means the user dropped the stream early —
        // so we call `finalization_fn` if it’s still present.
        let finalized = self.finalized.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |_| Some(true)).is_ok();
        if !finalized {
            if let Some(finalization_fn) = self.finalization_fn.take() {
                let handle = tokio::runtime::Handle::current();
                let _guard = handle.enter();
                handle.spawn(finalization_fn());
            }
        }
    }
}

/// The extension trait that adds `.on_complete_or_cancellation(...)` to all `Stream`s.
///
/// To use it, do:
///    use ogre_stream_ext::StreamExtFinalizationCallback;
///    use futures::StreamExt; // if you also want `.map()`, `.filter()`, etc.
///
/// Then:
///    let lock = ...;
///    mystream
///       .map(|x| …)
///       .on_complete_or_cancellation(move || future::ready(drop(lock)))
pub trait StreamExtFinalizationCallback: Stream + Sized {
    fn on_complete_or_cancellation<FinalizationFn, FnFut>(
        self,
        finalization_fn: FinalizationFn,
    ) -> StreamWithFinalizationCallback<Self, FinalizationFn, FnFut>
    where
        FinalizationFn: FnOnce() -> FnFut,
        FnFut: Future<Output=()> + Send + 'static,
    {
        StreamWithFinalizationCallback::new(self, finalization_fn)
    }
}

impl<S: Stream> StreamExtFinalizationCallback for S {}
