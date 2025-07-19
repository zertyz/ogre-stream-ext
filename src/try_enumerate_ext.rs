//! Adds both `.try_enumerate()` and `.try_enumerate_all()` to Stream's, filling in this gap left by the current `futures::TryStreamExt` implementation

use futures::{StreamExt, stream::Map, Stream};

pub trait TryEnumerateExt<Ok, Err>: Stream<Item=Result<Ok, Err>> + Sized {

    /// Like `StreamExt::enumerate`, but yields `Result<(idx, ok), err>`.
    /// See also [TryEnumerateAllExt::try_enumerate_all()] if you also want the count for `Err` results.
    #[allow(clippy::type_complexity)]
    fn try_enumerate(
        self
    ) -> Map<
        futures::stream::Enumerate<Self>,
        fn((usize, Result<Ok, Err>)) -> Result<(usize, Ok), Err>,
    > {
        self.enumerate().map(|(i, res)| res.map(|v| (i, v)))
    }
}
impl<S, Ok, Err> TryEnumerateExt<Ok, Err> for S where S: Stream<Item=Result<Ok, Err>> + Sized {}

pub trait TryEnumerateAllExt<Ok, Err>: Stream<Item=Result<Ok, Err>> + Sized {

    /// Like [TryEnumerateExt::try_enumerate()], but yields `Result<(idx, ok), (idx, err)>`.
    #[allow(clippy::type_complexity)]
    fn try_enumerate_all(
        self
    ) -> Map<
        futures::stream::Enumerate<Self>,
        fn((usize, Result<Ok, Err>)) -> Result<(usize, Ok), (usize, Err)>,
    > {
        self.enumerate().map(|(i, res)| res.map(|v| (i, v)).map_err(|err| (i, err)))
    }
}
impl<S, Ok, Err> TryEnumerateAllExt<Ok, Err> for S where S: Stream<Item=Result<Ok, Err>> + Sized {}


#[cfg(test)]
mod tests {
    use std::future;
    use super::*;
    use futures::stream;
    use tokio::pin;

    /// Assures the enumeration is done for yielded items,
    /// regardless if they are Ok or Err results
    #[tokio::test]
    async fn enumerate_yielded_regardless_of_errors() {
        let stream = stream::iter([Ok(0), Err(1), Ok(2)])
            .try_enumerate()
            .filter_map(|res| future::ready(res.ok()));
        pin!(stream);
        for i in 0..2 {
            let (enumerate_idx, value) = stream.next().await.expect("Stream ended prematurely");
            assert_eq!(enumerate_idx, value, "Enumeration index mismatch for item #{i}")
        }

    }
}