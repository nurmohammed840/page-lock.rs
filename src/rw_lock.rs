use super::*;

#[derive(Default)]
struct RefCounter {
    count: usize,
    wakers: Vec<(*mut PollState, Waker)>,
}

unsafe impl Send for RefCounter {}
unsafe impl Sync for RefCounter {}

#[derive(Default)]
pub struct RwLock<T> {
    mutex: Mutex<T>,
    readers: HashMap<T, RefCounter>,
}

unsafe impl<T: Send> Send for RwLock<T> {}
unsafe impl<T: Sync> Sync for RwLock<T> {}

impl<T: Eq + Hash + Copy + Unpin> RwLock<T> {
    #[inline]
    pub fn new() -> RwLock<T> {
        RwLock {
            mutex: Mutex::new(),
            readers: HashMap::new(),
        }
    }

    pub async fn read(&self, num: T) -> ReadGuard<'_, T> {
        self.mutex.until_unlocked(num).await;
        self.readers
            .write(num)
            .entry()
            .or_insert_with(RefCounter::default)
            .count += 1;

        ReadGuard {
            num,
            readers: &self.readers,
        }
    }

    pub async fn write(&self, num: T) -> WriteGuard<'_, T> {
        let guard = self.mutex.lock(num).await;
        UntilAllReaderDropped {
            num,
            readers: &self.readers,
            state: PollState::Init,
        }
        .await;
        guard
    }
}

pub struct ReadGuard<'a, T: Eq + Hash + Copy> {
    num: T,
    readers: &'a HashMap<T, RefCounter>,
}

// impl<T: ?Sized> !Send for ReadGuard<'_, T> {}
// impl<T: ?Sized + Sync> Sync for ReadGuard<'_, T> {}

impl<'a, T: Eq + Hash + Copy> Drop for ReadGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            let mut cell = self.readers.write(self.num);
            let mut rc = cell.get_mut().unwrap_unchecked();
            rc.count -= 1;
            if rc.count == 0 {
                let rc = cell.remove().unwrap_unchecked();
                for (state, waker) in rc.wakers {
                    *state = PollState::Ready;
                    waker.wake();
                }
            }
        }
    }
}

struct UntilAllReaderDropped<'a, T> {
    num: T,
    state: PollState,
    readers: &'a HashMap<T, RefCounter>,
}

impl<T: Unpin + Hash + Eq + Copy> Future for UntilAllReaderDropped<'_, T> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        ret_fut!(this.state, {
            match this.readers.write(this.num).get_mut() {
                Some(rc) => rc.wakers.push((&mut this.state, cx.waker().clone())),
                None => return Poll::Ready(()),
            }
        });
    }
}
