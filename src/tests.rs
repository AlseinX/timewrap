use crate::{async_fn, Timewrap};

#[test]
fn it_works() {
    let mut timewrap = Timewrap::new();
    timewrap.spawn(async_fn!(|timewrap| {
        timewrap.spawn(move |timewrap| {
            Box::pin(async move {
                for i in 0..10 {
                    timewrap.delay(10).await;
                    println!("a {}", i);
                }
            })
        });
        timewrap.delay(5).await;
        for i in 0..10 {
            timewrap.delay(10).await;
            println!("b {}", i);
        }
    }));
    timewrap.drive_block(200);
}
