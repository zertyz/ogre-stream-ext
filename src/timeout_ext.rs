//! Adds `.close_stream_on_item_timeout(max_duration_for_next_element)` to `Stream`s,
//! closing the Stream immediately after yielding the timeout error if no items arrive on time.
//!
//! NOTE: although the crate `futures_time` has a `.timeout()` method for Streams -- which appears to
//!       give what we want -- it does not: it indeed yields a timeout error after a duration has
//!       elapsed **after** the last element has been yielded, but it doesn't close the Stream,
//!       still allowing further elements to be yielded.

use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use async_io::Timer;
use futures::Stream;

#[derive(Debug)]
pub struct ItemTimeoutErr {
    pub previous_instant: Instant,
}
impl Display for ItemTimeoutErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        <Self as Debug>::fmt(self, f)
    }
}
impl std::error::Error for ItemTimeoutErr {}

/// The extension trait that adds `.close_stream_on_item_timeout(max_duration_for_next_element)` to all `Stream`s.
///
/// To use it, do:
///    use crate::stream_timeout::StreamExtCloseOnItemTimeout;
///    use futures::StreamExt; // if you also want `.map()`, `.filter()`, etc.
///
/// Then:
///    my_stream
///       .map(|x| …)
///       .close_stream_on_item_timeout(Duration::from_secs(...));
///      .filter_map(|timeout_result| match timeout_result {
///        Ok(original_element) => Some(original_element),  // unwrap and yield the original element downstream
///        Err(_timeout_err) => {
///          // do whatever you need when the timeout is detected and the stream is about to be closed
///          None  // do not yield the timeout error downstream
///        }
///      })
pub trait StreamExtCloseOnItemTimeout: Stream + Sized {

    fn close_stream_on_item_timeout(
        self,
        timeout: Duration,
    ) -> StreamWithItemTimeout<Self> {
        StreamWithItemTimeout::new(self, timeout)
    }
}

impl<S: Stream> StreamExtCloseOnItemTimeout for S {}


/// A Stream wrapper that will yield a timeout error if an item is not generated
/// within a maximum specified time, closing the Stream in the next `.poll()`
pub struct StreamWithItemTimeout<UpstreamType>
where
    UpstreamType: Stream,
{
    upstream: UpstreamType,
    timeout: Duration,
    timer: async_io::Timer,
    /// Flag: did the timeout happen? Close on the next `.poll()`
    timedout: AtomicBool,
}

impl<UpstreamType> StreamWithItemTimeout<UpstreamType>
where
    UpstreamType: Stream,
{
    pub fn new(upstream: UpstreamType, timeout: Duration) -> Self {
        StreamWithItemTimeout {
            upstream,
            timeout,
            timer: Timer::after(timeout),
            timedout: AtomicBool::new(false),
        }
    }
}

impl<UpstreamType, ItemType> Stream for StreamWithItemTimeout<UpstreamType>
where
    UpstreamType: Stream<Item = ItemType> + Unpin,
{
    type Item = Result<ItemType, ItemTimeoutErr>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.timedout.load(Relaxed) {
            // A timeout happened earlier. Close the stream regardless if there are more elements
            return Poll::Ready(None)
        }
        let timeout = self.timeout;
        match Pin::new(&mut self.upstream).poll_next(cx) {
            Poll::Ready(Some(item)) => {
                // reset the timer & return the element
                _ = std::mem::replace(&mut self.timer, Timer::after(timeout));
                Poll::Ready(Some(Ok(item)))
            },

            Poll::Ready(None) => {
                // stream ended spontaneously without any timeout
                Poll::Ready(None)
            }

            Poll::Pending => {
                // no element available in the stream -- check for timeout
                match Pin::new(&mut self.timer).poll(cx) {
                    Poll::Pending => Poll::Pending,     // didn't time out yet
                    Poll::Ready(instant) => {
                        // the timer fired -- we have a timeout: yield the error and schedule for stream termination
                        self.timedout.store(true, Relaxed);
                        Poll::Ready(Some(Err(ItemTimeoutErr { previous_instant: instant })))
                    },
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use futures::{SinkExt, StreamExt};

    /// Asserts our basic timeout functionality works as expected:
    ///   1. If an element takes more than the timeout duration to be yielded, the timeout error will be yielded instead
    ///   2. After the timeout error is yielded, the stream will be closed immediately, regardless if there are more elements available
    #[tokio::test]
    async fn basic_timeout_requirements() {
        let (mut tx, rx) = futures::channel::mpsc::channel(0);
        let mut out_stream = rx
            .boxed()
            .close_stream_on_item_timeout(Duration::from_millis(100));  // set the timeout for yielded items

        // task to publish items -- item of value '10' (and beyond) should not be received
        _ = tokio::spawn(async move {
            for i in 0..15 {
                tokio::time::sleep(Duration::from_millis(((i as f64)*10.1) as u64)).await;
                tx.send(i).await.expect("Error sending an element");
            }
        });

        // consume the 10 OK results (items from 0 to 9)
        for expected_item in 0..=9 {
            let observed_item = out_stream.next().await
                .unwrap_or_else(|| panic!("Stream ended prematurely at #{expected_item}"))
                .unwrap_or_else(|err| panic!("Timeout happened prematurely at #{expected_item}: {err}"));
            assert_eq!(observed_item, expected_item, "Received item is wrong");
        }

        // consume the timeout result (item of value 10 and beyond takes more than 1 second to be yielded)
        let observed_timeout_result = out_stream.next().await
            .expect("Stream ended prematurely -- without yielding the Timeout error");
        assert!(observed_timeout_result.is_err(), "item of value '10' was yielded without timing out. Yielded result: {observed_timeout_result:?}");

        // assert the stream ended -- even when there were other elements available
        assert!(out_stream.next().await.is_none(), "Stream did not end after a timeout was detected");
    }

    /// Asserts the timeout still happens if the first elements takes so much to be yielded
    #[tokio::test]
    async fn timeout_before_first_element() {

        const TIMEOUT: Duration = Duration::from_millis(100);

        let (_tx, rx) = futures::channel::mpsc::channel::<()>(0);
        let mut out_stream = rx
            .boxed()
            .close_stream_on_item_timeout(TIMEOUT);  // set the timeout for yielded items

        // no elements will be published on the stream

        // consume -- a single timeout item will be yielded after `TIMEOUT` has elapsed
        let stopwatcher = Instant::now();
        let observed_result = out_stream.next().await
            .expect("Stream ended prematurely -- without yielding the Timeout error");
        assert!(observed_result.is_err(), "an item was yielded without timing out. Yielded result: {observed_result:?}");
        let elapsed_time = stopwatcher.elapsed();
        assert!((TIMEOUT.as_secs_f64() - elapsed_time.as_secs_f64()).abs() < 1e-3, "The Timeout error did not happen at the right time");

        // assert the stream ended
        assert!(out_stream.next().await.is_none(), "Stream did not end after a timeout was detected");
    }

    /// Asserts we are still able to use the Stream without ever timing out
    #[tokio::test]
    async fn regular_stream_usage() {
        let (mut tx, rx) = futures::channel::mpsc::channel(0);
        let mut out_stream = rx
            .boxed()
            .close_stream_on_item_timeout(Duration::from_millis(100));  // set the timeout for yielded items

        // task to publish items -- items will get closer to the timeout, but never reach it
        _ = tokio::spawn(async move {
            for i in 0..15 {
                tokio::time::sleep(Duration::from_millis((((i % 10) as f64)*10.1) as u64)).await;
                tx.send(i).await.expect("Error sending an element");
            }
            // `tx` was moved to this task. Once it ends, the Stream will be closed.
            // So the following is not really necessary
            tx.close_channel();
        });

        // consume the 15 OK results
        for expected_item in 0..15 {
            let observed_item = out_stream.next().await
                .unwrap_or_else(|| panic!("Stream ended prematurely at #{expected_item}"))
                .unwrap_or_else(|err| panic!("Timeout happened prematurely at #{expected_item}: {err}"));
            assert_eq!(observed_item, expected_item, "Received item is wrong");
        }

        // sanity check: assert the stream really ended
        assert!(out_stream.next().await.is_none(), "Sanity check failed: Stream did not end");
    }

}