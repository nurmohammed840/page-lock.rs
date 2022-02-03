use super::*;

#[derive(Debug, Default)]
struct RefCounter {
    count: usize,
    wakers: Vec<Waker>,
}

pub struct RwLock<T> {
    write_lock: Mutex<T>,
    readers: DashMap<T, RefCounter>,
}

impl<T: Eq + Hash + Copy + Unpin> RwLock<T> {
    pub fn new() -> RwLock<T> {
        RwLock {
            write_lock: Mutex::new(),
            readers: DashMap::new(),
        }
    }

    pub async fn read(&self, num: T) -> ReadGuard<'_, T> {
        self.write_lock.wait_for_unlock(num).await;
        self.readers
            .entry(num)
            .or_insert(RefCounter::default())
            .count += 1;

        ReadGuard {
            num,
            readers: &self.readers,
        }
    }

    pub async fn write(&self, num: T) -> WriteGuard<'_, T> {
        let write_guard = self.write_lock.lock(num).await;
        UntilAllReaderDropped::new(num, &self.readers).await;
        write_guard
    }
}

pub struct ReadGuard<'a, T: Eq + Hash> {
    num: T,
    readers: &'a DashMap<T, RefCounter>,
}

impl<'a, T: Eq + Hash> Drop for ReadGuard<'a, T> {
    fn drop(&mut self) {
        let count = unsafe {
            let mut rc = self.readers.get_mut(&self.num).unwrap_unchecked();
            rc.count -= 1;
            rc.count
        };
        if count == 0 {
            let (_, rc) = unsafe { self.readers.remove(&self.num).unwrap_unchecked() };
            for waker in rc.wakers {
                waker.wake();
            }
        }
    }
}

struct UntilAllReaderDropped<'a, T> {
    num: T,
    state: bool,
    readers: &'a DashMap<T, RefCounter>,
}

impl<'a, T> UntilAllReaderDropped<'a, T> {
    fn new(num: T, readers: &'a DashMap<T, RefCounter>) -> Self {
        Self {
            num,
            readers,
            state: false,
        }
    }
}

impl<T: Unpin + Hash + Eq> Future for UntilAllReaderDropped<'_, T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.state {
            return Poll::Ready(());
        }
        match this.readers.get_mut(&this.num) {
            Some(mut rc) => rc.wakers.push(cx.waker().clone()),
            None => return Poll::Ready(()),
        }
        this.state = true;
        Poll::Pending
    }
}
