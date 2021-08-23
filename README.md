# Timewrap

This is a rust library that supports a virtual timeline with capability to Rust's async ecosystem. Within it, the users may execute async task that could `await` for specified virtual time, and would run sequentially by the `await`ed time. It does not actually wait for time in the real world, but just ensures the sequence of execution by the given virtual time.

It could be useful for writing game or simulation logic that has a time dimension decoupled with the real time, with concise async coroutine coding style, instead of event handling, manually written state machine, or callback hell.

## How can I use it

The very simple demo could be found in the [unit test](./src/tests.rs).

```rust
// Initialize a Timewrap instance with an attached state.
let result = Mutex::new(String::new());
let timewrap = Timewrap::new_with_state(result);

// Spawn just like you do with `tokio`, but does not need to be 'static.
timewrap.spawn(async_fn!(|timewrap| {
    use std::fmt::Write;

    // Spawn another task within a running task.
    timewrap.spawn(async {
        for i in 0..3 {
            timewrap.delay(10).await;
            let mut result = timewrap.lock().await;
            write!(result, "a{}", i).unwrap();
        }
    });

    timewrap.delay(5).await;

    for i in 0..3 {
        timewrap.delay(10).await;
        let mut result = timewrap.lock().await;
        write!(result, "b{}", i).unwrap();
    }
}));

// Drive the Timewrap to the estimated time synchronously.
timewrap.drive_shared_block(35);

// Consume the result.
let result = timewrap.into_state().into_inner();
assert_eq!(result, "a0b0a1b1a2b2");
```