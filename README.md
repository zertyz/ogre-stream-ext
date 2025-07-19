# `ogre-stream-ext`: `futures` shortcuts with ergonomic extensions to unlock the full power of Streams in Rust

This crate extends the `futures` Streams, adding the needed functionalities to enable useful and neat patterns when working with Streams for
event driven and reactive programming.


## Alpha Status
We are still in early infancy with still some unstable & experimental APIs, missing docs, and even non-optimized implementations.


## Additional "administrative events" callbacks:
 * Stream Finalization -- Introduces a "on close" & "on cancellation" callbacks to give notice that the Stream is ending.
                          The "on close" callback is able to yield an optional final item to the Stream.
 * Item production Timeout -- closes the Stream if new items do not arrive within some time limit, optionally yielding a final item
                              (possibly a Timeout Error).

## Misc features
 * Items Buffer -- Allows the accumulation of some items so the producer is not slown down if the consumer glitches a little.
                   Also used to reduce the pipeline latency -- when buffer is 1.
 * Rate Keeper -- tight control over the maximum items flowing through the Stream, to preserve computational resources.
                  This implementation is more precise and more optimized than the one offered by `whatever-trait` --
                  please see the benchmarks -- and yet more flexible in regard to keeping up the pace if temporary slowdowns occur.
 * `try_` extensions -- multiple new `.try_` combinators simply missing from the `features::TryStreamExt` extension, such as:
   * `.try_enumerate()` & `.try_enumerate_all()`