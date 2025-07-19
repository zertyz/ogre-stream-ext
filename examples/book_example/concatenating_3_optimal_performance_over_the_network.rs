//! This examples demonstrates a concatenation of streams coming from the network with the following characteristics:
//!   1) A driver stream is started -- `storage_accounts_stream`;
//!   2) For every element in `storage_accounts_stream`, we will start other 2 streams: `blobs_stream` and `trees_stream`
//!      -- which takes the storage account element as parameter;
//!   3) Everything is concatenated into a single output stream -- all elements of `storage_accounts_stream`, `blobs_stream` and `trees_stream`;
//!   4) For every input Stream, there are 3 elements that are yielded at 1 per second speed -- to simulate that they are coming from the network.
//!
//! From the above, it turns out that these characteristics can be executed in a theoretical minimum of 3 * 3 = 9 seconds
//! -- and the following code demonstrates how to achieve this performance.
//!
//! Explanation:
//!   * .buffer_unordered(n) allows to resolve n .map(async) operations at a time. Since we know the input speeds of `storage_accounts_stream`,
//!     we could use n = 1, but in real world scenarios, we don't have such consistent speeds, so we may allow more elements to be processed
//!     simultaneously.
//!   * `flatten_unordered(3) allows flattening up to 3 elements of the inner stream at a time. By using 3, we are ensuring we
//!     will include, simultaneously, the already resolved `storage_account_n` and the 2 newly started streams `blobs_stream` and `trees_stream`.
//!     Each element of the newly started streams are yielded at 1 per second speed, but both streams are started simultaneously -- meaning
//!     the flattening will be able to call for the next element of both streams concurrently -- for a total of 2 to 3 elements per second:
//!     2 elements per second when resolving the inner `blobs_stream` and `trees_stream` elements; 3 elements per second when a new element
//!     of `storage_accounts_stream` is available.

use std::time::Instant;
use futures::{stream, Stream, StreamExt};

const MAX_CONCURRENCY: usize = 6;


#[tokio::main]
async fn main() {
    let storage_accounts_stream = upgrade_to_throttled_stream(storage_accounts_stream().await).await;
    let mut final_stream = storage_accounts_stream
        .map(|storage_account_n| async move {
            let blobs_stream = upgrade_to_throttled_stream(blobs_stream(storage_account_n).await).await;
            let trees_stream = upgrade_to_throttled_stream(trees_stream(storage_account_n).await).await;
            stream::iter([storage_account_n])
                .chain(blobs_stream)
                .chain(trees_stream)
        })
        .buffer_unordered(MAX_CONCURRENCY)
        .flatten_unordered(3);  // fixed because we are concatenating 3 streams

    let start = Instant::now();
    final_stream.for_each(|v| async move {
        println!("v = {v}");
    }).await;
    println!("Execution time: {:?}", start.elapsed());
}

async fn storage_accounts_stream() -> impl Stream<Item=usize> {
    stream::iter([10, 20, 30])
}

async fn blobs_stream(storage_account_n: usize) -> impl Stream<Item=usize> {
    stream::iter([storage_account_n+1, storage_account_n+2, storage_account_n+3])
}

async fn trees_stream(storage_account_n: usize) -> impl Stream<Item=usize> {
    stream::iter([storage_account_n+4, storage_account_n+5, storage_account_n+6])
}

/// Simulates the given `stream` is coming from the network: elements are yielded at 1 per second speed
async fn upgrade_to_throttled_stream(stream: impl Stream<Item=usize>) -> impl Stream<Item=usize> {
    stream
        .map(|element| async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            element
        })
        .buffer_unordered(1)    // resolves 1 future at a time -- effectively yielding 1 element per second
}