//! Naive approach execution time: 3 * (1 + 3 + 1) = 15 seconds
//! Intermediary approach: 1 * (1 + 3 + 1) = 5 seconds
//! Best possible: 3 + (1 + 3) = 7 seconds

use std::future;
use std::time::Instant;
use futures::{stream, Stream, StreamExt};
use tokio::time::{sleep, Duration};

async fn fetch_alumni_ids() -> impl Stream<Item=u32> {
    upgrade_to_throttled_stream(stream::iter([1, 2, 3])).await
}
async fn fetch_profile(alumni_id: u32) -> u128 {
    sleep(Duration::from_millis(1000)).await; // Simulate delay for data over the network
    const SAMPLE_PROFILE: u128 = 83475847849;
    SAMPLE_PROFILE
}
async fn fetch_grades(alumni_id: u32) -> impl Stream<Item=f64> {
    upgrade_to_throttled_stream(stream::iter([90.0, 85.0, 88.0])).await
}
#[derive(Debug)]
enum AlumniData {
    Profile {id: u32, profile: u128 },
    GradesAverage {id: u32, grades_average: f64 },
}
#[tokio::main]
async fn main() {
    let mut transformed_stream = fetch_alumni_ids().await
        .map(|alumni_id| async move {
            let profile = fetch_profile(alumni_id).await;
            let grades_average = fetch_grades(alumni_id).await
                .enumerate()
                .fold(0.0, |avg, (i, grade)| future::ready(avg + (grade - avg) / (i+1) as f64) ).await;
            stream::iter([AlumniData::GradesAverage { id: alumni_id, grades_average }])
                .chain(stream::iter([AlumniData::Profile { id: alumni_id, profile }]))
        })
        .buffer_unordered(1)
        .flatten_unordered(1);

    let start = Instant::now();
    while let Some(alumni_data) = transformed_stream.next().await {
        println!("{alumni_data:?}");
    }
    println!("Execution time: {:?}", start.elapsed());
}

/// Simulates the given `stream` is coming from the network: elements are yielded at 1 per second speed
async fn upgrade_to_throttled_stream<T>(stream: impl Stream<Item=T>) -> impl Stream<Item=T> {
    stream
        .map(|element| async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            element
        })
        .buffer_unordered(1)    // resolves 1 future at a time -- effectively yielding 1 element per second
}