use parking_lot::{Condvar, Mutex};
use std::sync::Arc;

#[derive(Clone)]
pub struct Semaphore {
    inner: Arc<Inner>,
}

struct Inner {
    count: Mutex<usize>,
    cvar: Condvar,
    max: usize,
}

impl Semaphore {
    pub fn new(max: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                count: Mutex::new(0),
                cvar: Condvar::new(),
                max,
            }),
        }
    }

    pub fn acquire(&self) -> SemaphoreGuard {
        let mut count = self.inner.count.lock();
        while *count >= self.inner.max {
            self.inner.cvar.wait(&mut count);
        }
        *count += 1;
        SemaphoreGuard {
            inner: Arc::clone(&self.inner),
        }
    }

    pub fn try_acquire(&self) -> Option<SemaphoreGuard> {
        let mut count = self.inner.count.lock();
        if *count < self.inner.max {
            *count += 1;
            Some(SemaphoreGuard {
                inner: Arc::clone(&self.inner),
            })
        } else {
            None
        }
    }

    pub fn available_permits(&self) -> usize {
        self.inner.max - *self.inner.count.lock()
    }

    pub fn max_permits(&self) -> usize {
        self.inner.max
    }
}

pub struct SemaphoreGuard {
    inner: Arc<Inner>,
}

impl Drop for SemaphoreGuard {
    fn drop(&mut self) {
        let mut count = self.inner.count.lock();
        *count -= 1;
        self.inner.cvar.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[test]
    fn test_semaphore_basic() {
        let sem = Semaphore::new(2);
        let g1 = sem.acquire();
        let g2 = sem.acquire();
        assert_eq!(sem.available_permits(), 0);
        drop(g1);
        assert_eq!(sem.available_permits(), 1);
        drop(g2);
        assert_eq!(sem.available_permits(), 2);
    }

    #[test]
    fn test_semaphore_concurrency() {
        let sem = Semaphore::new(3);
        let counter = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let sem = sem.clone();
            let counter = Arc::clone(&counter);
            let max_concurrent = Arc::clone(&max_concurrent);
            handles.push(thread::spawn(move || {
                let _guard = sem.acquire();
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                max_concurrent.fetch_max(current, Ordering::SeqCst);
                thread::sleep(std::time::Duration::from_millis(10));
                counter.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert!(max_concurrent.load(Ordering::SeqCst) <= 3);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        assert_eq!(sem.available_permits(), 3);
    }

    #[test]
    fn test_semaphore_try_acquire() {
        let sem = Semaphore::new(1);
        let g1 = sem.try_acquire();
        assert!(g1.is_some());
        let g2 = sem.try_acquire();
        assert!(g2.is_none());
        drop(g1);
        let g3 = sem.try_acquire();
        assert!(g3.is_some());
    }
}
