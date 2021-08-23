use crate::{async_fn, Timewrap};
use futures::lock::Mutex;

#[test]
fn it_works() {
    let result = Mutex::new(String::new());
    let timewrap = Timewrap::new_with_state(result);
    timewrap.spawn(async_fn!(|timewrap| {
        use std::fmt::Write;
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
    timewrap.drive_shared_block(35);
    let result = timewrap.into_state().into_inner();
    assert_eq!(result, "a0b0a1b1a2b2");
}
