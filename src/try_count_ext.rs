//! Adds `.try_count()` to Stream's, not provided by either `futures` nor `futures-util` crates

use std::future::Future;
use futures::{StreamExt, stream::Map, Stream, TryStreamExt, TryStream};

pub trait TryCountExt: TryStream + Sized {

    /// A terminal operation that counts the number of `Ok` items in the Stream,
    /// erroring if an `Err` `Result` is ever encountered
    fn try_count(
        self
    ) -> impl Future<Output=Result<usize, Self::Error>> {
        self.try_fold(0usize, |acc, _| async move {
            Ok(acc + 1)
        })
    }
}
impl<S> TryCountExt for S where S: TryStream + Sized {}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[tokio::test]
    async fn try_count_ok() {
        let items = [Ok::<_, ()>(0), Ok(1), Ok(2)];
        let observed_count = stream::iter(items)
            .try_count()
            .await
            .expect("`try_count()` should not fail for this test");
        assert_eq!(observed_count, items.len(), "Wrong count")

    }

    #[tokio::test]
    async fn try_count_err() {
        let items = [Ok(0), Err(1), Ok(2)];
        let observed_result = stream::iter(items)
            .try_count()
            .await;
        assert!(observed_result.is_err(), "`try_count()` should have resulted in `Err` for this test");

    }
}