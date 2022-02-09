use super::*;

#[derive(Default)]
struct RefCounter {
    count: usize,
    wakers: Vec<Waker>,
}

pub struct RwLock<T> {
    write_lock: Mutex<T>,
    readers: HashMap<T, RefCounter>,
}

impl<T: Eq + Hash + Copy + Unpin> RwLock<T> {
    pub fn new() -> RwLock<T> {
        RwLock {
            write_lock: Mutex::new(),
            readers: HashMap::new(),
        }
    }

    pub async fn read(&self, num: T) -> ReadGuard<'_, T> {
        self.write_lock.wait_for_unlock(num).await;
        self.readers
            .write(num)
            .entry()
            .or_insert(RefCounter::default())
            .count += 1;

        ReadGuard {
            num,
            readers: &self.readers,
        }
    }

    pub async fn write(&self, num: T) -> WriteGuard<'_, T> {
        let write_guard = self.write_lock.lock(num).await;
        UntilAllReaderDropped {
            num,
            readers: &self.readers,
            state: false,
        }
        .await;
        write_guard
    }
}

pub struct ReadGuard<'a, T: Eq + Hash + Copy> {
    num: T,
    readers: &'a HashMap<T, RefCounter>,
}

impl<'a, T: Eq + Hash + Copy> Drop for ReadGuard<'a, T> {
    fn drop(&mut self) {
        let mut readers = self.readers.write(self.num);
        let mut rc = unsafe { readers.get_mut().unwrap_unchecked() };
        rc.count -= 1;
        if rc.count == 0 {
            let rc = unsafe { readers.remove().unwrap_unchecked() };
            for waker in rc.wakers {
                waker.wake();
            }
        }
    }
}

struct UntilAllReaderDropped<'a, T> {
    num: T,
    state: bool,
    readers: &'a HashMap<T, RefCounter>,
}

impl<T: Unpin + Hash + Eq + Copy> Future for UntilAllReaderDropped<'_, T> {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.state {
            return Poll::Ready(());
        }
        match self.readers.write(self.num).get_mut() {
            Some(rc) => rc.wakers.push(cx.waker().clone()),
            None => return Poll::Ready(()),
        }
        self.state = true;
        Poll::Pending
    }
}
