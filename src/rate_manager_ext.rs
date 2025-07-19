//! This should be re-implemented:
//!  - Add as a proper Stream Extension -- possibly erroring out if 2 of these are used in the same pipeline, as this would be inefficient
//!  - Stop using `futures_time` -- which is extremely inefficient and imprecise. Add a benchmark to prove both the inefficiency and the fragility regarding the producer latency.
//!
//! The proper & flexible solution will be two-fold, as the rate can be applied either up and/or downstream
//!  - By limiting the downstream yielding of elements, we may allow the producer to be occasionally ahead, which are buffered. This gives us a precise control of the items
//!    processing speed, as the small buffer would absorb production latencies.
//!  - The rate can be applied upstream, for cases where producing an item is expensive. In this case, the item processing speed will be a little smaller due to production
//!    latencies.
//!  - In rare circumstances, may both be used? Something to investigate.
//!
//! The real solution is really involved, as one can compose a pipeline containing more than one `.rate_limit()` call -- which would be inefficient. But, would it make sense?
//!
//! More often not. So writing a lint may be a good idea. Telling something like:
//!   "calling `.rate_limit()` on an already-throttled stream"
//!   "-- you probably only want one rate limit in your pipeline. In rare situations, it would make sense to have more than 1 call (like when composing Stream pipelines via optional code that are able to exhaust resources even further)"
//!   "   if this is the case, than use #[allow_multiple_stream_rate_limiters]"

use futures::{Stream, StreamExt};
use futures_time::time::Duration;

/// Takes the `input_stream` and the frequency `elements_per_second` and give out
/// a stream with the same elements, but restricted at yielding elements in that frequency, at most.
pub fn throttle_stream<ItemType, StreamType: Stream<Item = ItemType>>(
    input_stream: StreamType,
    elements_per_second: f64,
) -> impl Stream<Item = ItemType> {
    let duration_between_elements = Duration::from_secs_f64(1.0 / elements_per_second);
    let ticks = futures_time::stream::interval(duration_between_elements);
    input_stream.zip(ticks).map(|(item, _)| item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{stream, StreamExt};

    #[tokio::test]
    async fn constant_rate_stream() {
        let frequency = 50.0;
        let n_elements = 100;
        let expected_duration_secs = 2.0; // n_elements as f64 / frequency;
        let tolerance = 0.1;
        let unthrottled_stream = stream::iter(1..=n_elements);
        let throttled_stream = throttle_stream(unthrottled_stream, frequency);
        let start = std::time::Instant::now();
        let observed_n_elements = throttled_stream.count().await;
        let observed_duration = start.elapsed();
        assert_eq!(
            observed_n_elements, n_elements,
            "Number of elements doesn't match"
        );
        assert!(
            f64::abs(observed_duration.as_secs_f64() - expected_duration_secs) < tolerance,
            "Unexpected duration while consuming a throttled stream"
        );
    }
}
