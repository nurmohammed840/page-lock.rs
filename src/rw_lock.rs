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
    readers: Map<T, RefCounter>,
}

impl<T: Eq + Hash + Copy + Unpin> RwLock<T> {
    #[inline]
    pub fn new() -> RwLock<T> {
        RwLock {
            mutex: Mutex::new(),
            readers: new_map(),
        }
    }

    pub async fn read(&self, num: T) -> ReadGuard<'_, T> {
        self.mutex.until_unlocked(num).await;
        self.readers
            .lock()
            .unwrap()
            .entry(num)
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
            state: PollState::Init.into(),
        }
        .await;
        guard
    }
}

pub struct ReadGuard<'a, T: Eq + Hash + Copy> {
    num: T,
    readers: &'a Map<T, RefCounter>,
}

// impl<T: ?Sized> !Send for ReadGuard<'_, T> {}
// impl<T: ?Sized + Sync> Sync for ReadGuard<'_, T> {}

impl<'a, T: Eq + Hash + Copy> Drop for ReadGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            let mut map = self.readers.lock().unwrap();
            let mut rc = map.get_mut(&self.num).unwrap_unchecked();
            rc.count -= 1;
            if rc.count == 0 {
                let rc = map.remove(&self.num).unwrap_unchecked();
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
    state: UnsafeCell<PollState>,
    readers: &'a Map<T, RefCounter>,
}

impl<T: Unpin + Hash + Eq + Copy> Future for UntilAllReaderDropped<'_, T> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        ret_fut!(*this.state.get(), {
            let mut map = this.readers.lock().unwrap();
            match map.get_mut(&this.num) {
                Some(rc) => rc.wakers.push(( this.state.get(), cx.waker().clone())),
                None => return Poll::Ready(()),
            }
        });
    }
}
