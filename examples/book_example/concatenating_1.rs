// PATTERN: I want a Stream

use futures::{stream, Stream, StreamExt};

const MAX_CONCURRENCY: usize = 6;

#[tokio::main]
async fn main() {
    let mut storage_accounts = storage_account().await;

    let mut final_stream = storage_accounts
        .map(|storage_account_n| async move {
            stream::iter([storage_account_n])
                .chain(blob_stream(storage_account_n).await)
                .chain(tree_stream(storage_account_n).await)
        })
        .buffer_unordered(MAX_CONCURRENCY)
        .flatten_unordered(MAX_CONCURRENCY);

    while let Some(v) = final_stream.next().await {
        println!("v = {v}");
    }
}

async fn storage_account() -> impl Stream<Item=usize> {
    stream::iter([10, 20, 30])
}

async fn blob_stream(storage_account_n: usize) -> impl Stream<Item=usize> {
    stream::iter([storage_account_n+1, storage_account_n+2, storage_account_n+3])
}

async fn tree_stream(storage_account_n: usize) -> impl Stream<Item=usize> {
    stream::iter([storage_account_n+4, storage_account_n+5, storage_account_n+6])
}