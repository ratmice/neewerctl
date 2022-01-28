// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*
#![unstable(feature = "semaphore",
            reason = "the interaction between semaphores and the acquisition/release \
                      of resources is currently unclear",
            issue = "27798")]
#![rustc_deprecated(since = "1.7.0",
                    reason = "easily confused with system semaphores and not \
                              used enough to pull its weight")]
#![allow(deprecated)]
*/

use std::ops::Drop;
use std::sync::{Condvar, Mutex, TryLockError};
use std::{error, fmt};

/// A counting, blocking, semaphore.
///
/// Semaphores are a form of atomic counter where access is only granted if the
/// counter is a positive value. Each acquisition will block the calling thread
/// until the counter is positive, and each release will increment the counter
/// and unblock any threads if necessary.
///
/// # Examples
///
/// ```
/// #![feature(semaphore)]
///
/// use std::sync::Semaphore;
///
/// // Create a semaphore that represents 5 resources
/// let sem = Semaphore::new(5);
///
/// // Acquire one of the resources
/// sem.acquire();
///
/// // Acquire one of the resources for a limited period of time
/// {
///     let _guard = sem.access();
///     // ...
/// } // resources is released here
///
/// // Release our initially acquired resource
/// sem.release();
/// ```
pub struct Semaphore {
    lock: Mutex<isize>,
    cvar: Condvar,
}

/// An RAII guard which will release a resource acquired from a semaphore when
/// dropped.
pub struct SemaphoreGuard<'a> {
    sem: &'a Semaphore,
}

#[derive(Debug)]
pub enum TryAcquireError {
    WouldBlock,
}

impl<T> From<TryLockError<T>> for TryAcquireError {
    fn from(_err: TryLockError<T>) -> Self {
        TryAcquireError::WouldBlock
    }
}

impl fmt::Display for TryAcquireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "try_acquire failed because the operation would block".fmt(f)
    }
}

impl error::Error for TryAcquireError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

impl Semaphore {
    /// Creates a new semaphore with the initial count specified.
    ///
    /// The count specified can be thought of as a number of resources, and a
    /// call to `acquire` or `access` will block until at least one resource is
    /// available. It is valid to initialize a semaphore with a negative count.
    pub fn new(count: isize) -> Semaphore {
        Semaphore {
            lock: Mutex::new(count),
            cvar: Condvar::new(),
        }
    }

    /// Acquires a resource of this semaphore, blocking the current thread until
    /// it can do so.
    ///
    /// This method will block until the internal count of the semaphore is at
    /// least 1.
    pub fn acquire(&self) {
        let mut count = self.lock.lock().unwrap();
        while *count <= 0 {
            count = self.cvar.wait(count).unwrap();
        }
        *count -= 1;
    }

    /// Release a resource from this semaphore.
    ///
    /// This will increment the number of resources in this semaphore by 1 and
    /// will notify any pending waiters in `acquire` or `access` if necessary.
    pub fn release(&self) {
        *self.lock.lock().unwrap() += 1;
        self.cvar.notify_one();
    }

    /// Acquires a resource of this semaphore, returning an RAII guard to
    /// release the semaphore when dropped.
    ///
    /// This function is semantically equivalent to an `acquire` followed by a
    /// `release` when the guard returned is dropped.
    #[allow(unused)]
    pub fn access(&self) -> SemaphoreGuard {
        self.acquire();
        SemaphoreGuard { sem: self }
    }

    pub fn try_acquire(&self) -> Result<(), TryAcquireError> {
        let mut count = self.lock.try_lock()?;
        if *count <= 0 {
            return Err(TryAcquireError::WouldBlock);
        }
        *count -= 1;
        Ok(())
    }
}

impl<'a> Drop for SemaphoreGuard<'a> {
    fn drop(&mut self) {
        self.sem.release();
    }
}

#[cfg(test)]
mod tests {
    use std::prelude::v1::*;

    use super::Semaphore;
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_sem_acquire_release() {
        let s = Semaphore::new(1);
        s.acquire();
        s.release();
        s.acquire();
    }

    #[test]
    fn test_sem_basic() {
        let s = Semaphore::new(1);
        let _g = s.access();
    }

    #[test]
    fn test_sem_as_mutex() {
        let s = Arc::new(Semaphore::new(1));
        let s2 = s.clone();
        let _t = thread::spawn(move || {
            let _g = s2.access();
        });
        let _g = s.access();
    }

    #[test]
    fn test_sem_as_cvar() {
        /* Child waits and parent signals */
        let (tx, rx) = channel();
        let s = Arc::new(Semaphore::new(0));
        let s2 = s.clone();
        let _t = thread::spawn(move || {
            s2.acquire();
            tx.send(()).unwrap();
        });
        s.release();
        let _ = rx.recv();

        /* Parent waits and child signals */
        let (tx, rx) = channel();
        let s = Arc::new(Semaphore::new(0));
        let s2 = s.clone();
        let _t = thread::spawn(move || {
            s2.release();
            let _ = rx.recv();
        });
        s.acquire();
        tx.send(()).unwrap();
    }

    #[test]
    fn test_sem_multi_resource() {
        // Parent and child both get in the critical section at the same
        // time, and shake hands.
        let s = Arc::new(Semaphore::new(2));
        let s2 = s.clone();
        let (tx1, rx1) = channel();
        let (tx2, rx2) = channel();
        let _t = thread::spawn(move || {
            let _g = s2.access();
            let _ = rx2.recv();
            tx1.send(()).unwrap();
        });
        let _g = s.access();
        tx2.send(()).unwrap();
        rx1.recv().unwrap();
    }

    #[test]
    fn test_sem_runtime_friendly_blocking() {
        let s = Arc::new(Semaphore::new(1));
        let s2 = s.clone();
        let (tx, rx) = channel();
        {
            let _g = s.access();
            thread::spawn(move || {
                tx.send(()).unwrap();
                drop(s2.access());
                tx.send(()).unwrap();
            });
            rx.recv().unwrap(); // wait for child to come alive
        }
        rx.recv().unwrap(); // wait for child to be done
    }

    fn fight(semaphore: Arc<Semaphore>, max: usize) -> thread::JoinHandle<(i32, usize)> {
        thread::spawn(move || {
            let mut acquire_count = 0;
            let mut fail_acquire_count : usize = 0;
            let mut have_semaphore;
            loop {
                have_semaphore = semaphore.try_acquire();
                if let Ok(()) = have_semaphore {
                    std::thread::yield_now();
                    semaphore.release();
                    acquire_count += 1;
                    if acquire_count == 1000 || fail_acquire_count >= max {
                        return (acquire_count, fail_acquire_count);
                    }
                } else if fail_acquire_count < max - 1 {
                    fail_acquire_count += 1;
                } else {
                    return (acquire_count, fail_acquire_count);
                }
            }
        })
    }

    #[test]
    fn test_sem_try_acquire() {
        let s = Arc::new(Semaphore::new(2));
        let hndl1 = fight(s.clone(), std::usize::MAX);
        let hndl2 = fight(s.clone(), std::usize::MAX);
        let hndl3 = fight(s, std::usize::MAX);
        let a = hndl1.join().unwrap();
        let b = hndl2.join().unwrap();
        let c = hndl3.join().unwrap();
        println!("{:?} {:?} {:?}", a, b, c);
        assert_eq!(a.0, 1000);
        assert_eq!(b.0, 1000);
        assert_eq!(c.0, 1000);
    }

    #[test]
    fn test_sem_try_acquire_zero() {
        let s = Arc::new(Semaphore::new(0));
        let hndl5 = fight(s, 10);
        let be_zero = hndl5.join().unwrap();

        assert_eq!(be_zero.0, 0);
    }
}
