//! This examples demonstrates a concatenation of streams coming from the network with the following characteristics:
//!   1) A driver stream is started -- `storage_accounts_stream`
//!   2) For every element in `storage_accounts_stream`, we will start other 2 streams: `blobs_stream` and `trees_stream`
//!   3) Everything is concatenated into a single stream -- all elements of `storage_accounts_stream`, `blobs_stream` and `trees_stream`
//!   4) Whatever the stream, there are 3 elements that are yielded at 1 per second speed.
//!
//! This setup gives us a theoretical minimum execution time of 3 * 3 = 9 seconds.
//! However, the following example takes 3 * (1 + 3 + 3) = 21 seconds to run.

use std::time::Instant;
use futures::{stream, Stream, StreamExt};

const MAX_CONCURRENCY: usize = 1;

#[tokio::main]
async fn main() {
    let storage_accounts_stream = upgrade_to_delayed_stream(storage_accounts_stream().await).await;
    let mut final_stream = storage_accounts_stream
        .map(|storage_account_n| async move {
            let blobs_stream = upgrade_to_delayed_stream(blobs_stream(storage_account_n).await).await;
            let trees_stream = upgrade_to_delayed_stream(trees_stream(storage_account_n).await).await;
            stream::iter([storage_account_n])
                .chain(blobs_stream)
                .chain(trees_stream)
        })
        .buffer_unordered(MAX_CONCURRENCY)
        .flatten_unordered(MAX_CONCURRENCY);

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
async fn upgrade_to_delayed_stream(stream: impl Stream<Item=usize>) -> impl Stream<Item=usize> {
    stream
        .map(|storage_account_n| async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            storage_account_n
        })
        .buffer_unordered(1)
}