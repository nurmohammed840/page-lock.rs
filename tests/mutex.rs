use std::sync::Arc;
use page_lock::Mutex;
use tokio_test::assert_ready;
use tokio_test::task::spawn;

macro_rules! assert_pending {
    ($result: expr) => {
        assert!(matches!($result, std::task::Poll::Pending))
    };
}

#[test]
fn readiness() {
    let l1 = Arc::new(Mutex::new());
    let l2 = Arc::clone(&l1);
    let mut t1 = spawn(l1.lock(0));
    let mut t2 = spawn(l2.lock(0));

    let g = assert_ready!(t1.poll());

    // We can't now acquire the lease since it's already held in g
    assert_pending!(t2.poll());

    // But once g unlocks, we can acquire it
    drop(g);
    assert!(t2.is_woken());
    assert_ready!(t2.poll());
}