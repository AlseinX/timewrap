use crate::{async_fn, Timewrap};

#[test]
fn it_works() {
    let timewrap = Timewrap::new_with_state("x");
    timewrap.spawn(async_fn!(|timewrap| {
        timewrap.spawn(move |timewrap| {
            Box::pin(async move {
                for i in 0..10 {
                    timewrap.delay(10).await;
                    println!("a {} {}", i, timewrap.state());
                }
            })
        });
        timewrap.delay(5).await;
        for i in 0..10 {
            timewrap.delay(10).await;
            println!("b {} {}", i, timewrap.state());
        }
    }));
    timewrap.drive_shared_block(200);
}
