use core::sync::atomic::{AtomicU64, Ordering};

use common::{SyscallFutexLock, SyscallFutexUnlock};
use lock_api::{GuardNoSend, RawMutex};

use crate::syscalls::{syscall, syscall_get_thread_id};

pub struct RawBlockingLock(AtomicU64);

unsafe impl RawMutex for RawBlockingLock {
    const INIT: Self = Self(AtomicU64::new(0));

    type GuardMarker = GuardNoSend;

    fn lock(&self) {
        while !self.try_lock() {
            // unsafe { syscall::<SyscallFutexLock>(&(&self.0 as *const _ as u64)) };
        }
    }

    fn try_lock(&self) -> bool {
        self.0
            .compare_exchange(
                0,
                u64::from(u32::from(syscall_get_thread_id())),
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
    }

    unsafe fn unlock(&self) {
        self.0.store(0, Ordering::Release);
        // if self
        //     .0
        //     .compare_exchange(
        //         u64::from(u32::from(syscall_get_thread_id())),
        //         0,
        //         Ordering::AcqRel,
        //         Ordering::Relaxed,
        //     )
        //     .is_err()
        // {
        //     // unsafe { syscall::<SyscallFutexUnlock>(&(&self.0 as *const _ as u64)) };
        // }
    }
}

pub type BlockingLock<T> = lock_api::Mutex<RawBlockingLock, T>;
pub type BlockingLockGuard<'a, T> = lock_api::MutexGuard<'a, RawBlockingLock, T>;
