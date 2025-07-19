//! Consider a scenario where we process a Stream of Alumni and, for each id we generate 2 events: a profile
//! and the average grades – both querying additional data asynchronously. The transformed Stream outputs an
//! AlumniData enum containing either Profile or Grades.
//!
//! This example covers primarily the "Stream Enrichment Pattern" -- as additional asynchronous queries
//! are executed to compose the output elements -- but also contains the "Stream amplification pattern"
//! -- as 2 output elements are produced for every input element.
//!
//! The following code is optimized and takes 3 + (1 + 3) = 7 seconds to run.
//!
//! Can you do better than that? If not, why?
//!
//! As an exercise, there are less optimized approaches you can try to replicate:
//!  * Naive .map() approach execution time: 3 * (1 + 3 + 1) = 15 seconds -- without concurrency
//!  * Intermediary .map() approach: 3 + (1 + 3 + 1) = 9 seconds -- with partial concurrency
//!  * Naive Generator approach execution time: 3 * (1 + 3) = 12 seconds -- concurrency only on the queries


use std::future;
use std::time::Instant;
use futures::{stream, FutureExt, Stream, StreamExt};
use futures::stream::FuturesUnordered;
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
            let profile_fut = async move {
                let profile = fetch_profile(alumni_id).await;
                AlumniData::Profile { id: alumni_id, profile }
            };
            let grades_average_fut = async move {
                let grades_average = fetch_grades(alumni_id).await
                    .enumerate()
                    .fold(0.0, |avg, (i, grade)| future::ready(avg + (grade - avg) / (i+1) as f64) ).await;
                AlumniData::GradesAverage { id: alumni_id, grades_average }
            };
            FuturesUnordered::from_iter([
                profile_fut.boxed(),
                grades_average_fut.boxed(),
            ])
        })
        .buffer_unordered(16)    // put the number of concurrent composite operations here
        .flatten_unordered(2);                       //  this is fixed: the number of output events yielded for every input event

    let start = Instant::now();
    transformed_stream.for_each(|alumni_data| async move {
        println!("{alumni_data:?}");
    }).await;
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