use page_lock::RwLock;
use tokio_test::{assert_ready, task::spawn};

macro_rules! assert_pending {
    ($result: expr) => {
        assert!(matches!($result, std::task::Poll::Pending))
    };
}

#[test]
fn read_shared() {
    let rwlock = RwLock::new();

    let mut t1 = spawn(rwlock.read(0));
    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.read(0));
    assert_ready!(t2.poll());
}

// When there is an active shared owner, exclusive access should not be possible
#[test]
fn write_shared_pending() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.read(0));
    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.write(0));
    assert_pending!(t2.poll());
}

// When there is an active exclusive owner, subsequent exclusive access should not be possible
#[test]
fn read_exclusive_pending() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.write(0));

    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.read(0));
    assert_pending!(t2.poll());
}

// When there is an active exclusive owner, subsequent exclusive access should not be possible
#[test]
fn write_exclusive_pending() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.write(0));

    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.write(0));
    assert_pending!(t2.poll());
}

// When there is an active shared owner, exclusive access should be possible after shared is dropped
#[test]
fn write_shared_drop() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.read(0));

    let g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.write(0));
    assert_pending!(t2.poll());
    drop(g1);
    assert!(t2.is_woken());
    assert_ready!(t2.poll());
}

// when there is an active shared owner, and exclusive access is triggered,
// subsequent shared access should not be possible as write gathers all the available semaphore permits
#[test]
fn write_read_shared_pending() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.read(0));
    let _g1 = assert_ready!(t1.poll());

    let mut t2 = spawn(rwlock.read(0));
    assert_ready!(t2.poll());

    let mut t3 = spawn(rwlock.write(0));
    assert_pending!(t3.poll());

    let mut t4 = spawn(rwlock.read(0));
    assert_pending!(t4.poll());
}

// when there is an active shared owner, and exclusive access is triggered,
// reading should be possible after pending exclusive access is dropped
#[test]
fn write_read_shared_drop_pending() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.read(0));
    let _g1 = assert_ready!(t1.poll());

    let mut t2 = spawn(rwlock.write(0));
    assert_pending!(t2.poll());

    let mut t3 = spawn(rwlock.read(0));
    assert_pending!(t3.poll());
    drop(t2);

    assert!(t3.is_woken());
    assert_ready!(t3.poll());
}

