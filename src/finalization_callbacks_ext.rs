//! Adds a new Stream combinator able to call different functions once the Stream ends or is canceled
//!
//! Please see [crate::StreamWithFinalizationCallback] for a version that does not distinguish between
//! completion or cancellation.

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::Stream;

/// A Stream wrapper that can call two different closures:
/// 1) `complete_cb`: fired exactly once when the inner stream returns `None`.
/// 2) `cancel_cb`: fired exactly once if the wrapper is dropped before seeing `None`.
///
/// Internally, we keep each closure as an `Option<…>` so we can `take()` it
/// and invoke it just one time.
pub struct StreamWithFinalizationCallbacks<S, FComplete, FCancel>
where
    S: Stream,
    FComplete: FnOnce(),
    FCancel: FnOnce(),
{
    inner: S,
    complete_cb: Option<FComplete>,
    cancel_cb: Option<FCancel>,
    /// Flag: have we already called `complete_cb`? Once `true`, we must not call `cancel_cb`.
    finished: bool,
}

impl<S, FComplete, FCancel> StreamWithFinalizationCallbacks<S, FComplete, FCancel>
where
    S: Stream,
    FComplete: FnOnce(),
    FCancel: FnOnce(),
{
    /// Construct a new wrapper that:
    ///  - calls `complete_cb` once when `inner.poll_next()` returns `Ready(None)`, and
    ///  - calls `cancel_cb` if the wrapper is dropped before seeing `None`.
    ///
    /// If you don’t care about cancellations, pass in something like `|| {}` for `cancel_cb`.
    pub fn new(inner: S, complete_cb: FComplete, cancel_cb: FCancel) -> Self {
        StreamWithFinalizationCallbacks {
            inner,
            complete_cb: Some(complete_cb),
            cancel_cb: Some(cancel_cb),
            finished: false,
        }
    }
}

impl<S, FComplete, FCancel, T> Stream for StreamWithFinalizationCallbacks<S, FComplete, FCancel>
where
    S: Stream<Item = T> + Unpin,
    FComplete: FnOnce(),
    FCancel: FnOnce(),
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
                // The inner stream is done. If we have not yet called `complete_cb`, do so now.
                if !this.finished {
                    if let Some(cb) = this.complete_cb.take() {
                        cb();
                    }
                    this.finished = true;
                    // Clear the cancellation callback as well, so Drop won't fire it.
                    this.cancel_cb.take();
                }
                Poll::Ready(None)
            }

            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S, FComplete, FCancel> Drop for StreamWithFinalizationCallbacks<S, FComplete, FCancel>
where
    S: Stream,
    FComplete: FnOnce(),
    FCancel: FnOnce(),
{
    fn drop(&mut self) {
        // If we never reached the “finished” state, that means the user dropped the stream early —
        // so we call `cancel_cb` if it’s still present.
        if !self.finished {
            if let Some(cancel_cb) = self.cancel_cb.take() {
                cancel_cb();
            }
        }
        // (We do NOT call complete_cb here, because “complete” only happens on real EOF)
    }
}

/// A marker “do‐nothing” closure for the opposite callback:
///  - If the user only registers `.on_complete`, then `cancel_cb` is a no‐op.
///  - If the user only registers `.on_cancellation`, then `complete_cb` is a no‐op.
fn no_op() {}

/// The extension trait that adds `.on_complete(...)` and `.on_cancellation(...)` to all `Stream`s.
///
/// To use it, do:
///    use ogre_stream_ext::StreamExtFinalizationCallbacks;
///    use futures::StreamExt; // if you also want `.map()`, `.filter()`, etc.
///
/// Then:
///    mystream
///       .map(|x| …)
///       .on_complete(|| println!("done!"))
///       .on_cancellation(|| println!("cancelled early!"))
pub trait StreamExtFinalizationCallbacks: Stream + Sized {
    fn on_complete<FComplete>(
        self,
        complete_cb: FComplete,
    ) -> StreamWithFinalizationCallbacks<Self, FComplete, impl FnOnce()>
    where
        FComplete: FnOnce(),
    {
        StreamWithFinalizationCallbacks::new(self, complete_cb, no_op)
    }

    fn on_cancellation<FCancel>(
        self,
        cancel_cb: FCancel,
    ) -> StreamWithFinalizationCallbacks<Self, impl FnOnce(), FCancel>
    where
        FCancel: FnOnce(),
    {
        StreamWithFinalizationCallbacks::new(self, no_op, cancel_cb)
    }
}

impl<S: Stream> StreamExtFinalizationCallbacks for S {}
