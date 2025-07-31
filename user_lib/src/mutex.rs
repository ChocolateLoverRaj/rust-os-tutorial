use core::{
    num::NonZeroU32,
    sync::atomic::{AtomicU64, Ordering},
};

use common::{FUTEX_WAITERS, FutexLockError, SyscallFutexLock, SyscallFutexUnlock};
use lock_api::{GuardNoSend, RawMutex};

use crate::syscalls::{syscall, syscall_get_thread_id};

pub struct RawBlockingLock(AtomicU64);

unsafe impl RawMutex for RawBlockingLock {
    const INIT: Self = Self(AtomicU64::new(0));

    type GuardMarker = GuardNoSend;

    fn lock(&self) {
        let thread_id = syscall_get_thread_id().thread_id;
        let new = thread_id.get().into();
        let success = Ordering::Release;
        let failure = Ordering::Acquire;

        enum DoAction {
            TryExchangeZero,
            TryExchangeWaiters,
            Syscall,
        }
        let mut action = DoAction::TryExchangeZero;
        loop {
            match action {
                DoAction::TryExchangeZero => {
                    match self.0.compare_exchange(0, new, success, failure) {
                        Ok(_) => break,
                        Err(value) => {
                            if let Some(lock_owner) = NonZeroU32::new(value as u32)
                                && lock_owner == thread_id
                            {
                                break;
                            } else if value == FUTEX_WAITERS {
                                action = DoAction::TryExchangeWaiters;
                            } else {
                                action = DoAction::Syscall;
                            }
                        }
                    }
                }
                DoAction::TryExchangeWaiters => {
                    match self
                        .0
                        .compare_exchange(FUTEX_WAITERS, new, success, failure)
                    {
                        Ok(_) => break,
                        Err(value) => {
                            if value == 0 {
                                action = DoAction::TryExchangeZero;
                            } else {
                                action = DoAction::Syscall;
                            }
                        }
                    }
                }
                DoAction::Syscall => {
                    match unsafe { syscall::<SyscallFutexLock>(&(&self.0 as *const _ as u64)) } {
                        Ok(_) => break,
                        Err(FutexLockError::CheckWithWaiters) => {
                            action = DoAction::TryExchangeWaiters
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    fn try_lock(&self) -> bool {
        let thread_id = syscall_get_thread_id().thread_id;
        let new = thread_id.get().into();
        let success = Ordering::Release;
        let failure = Ordering::Acquire;
        loop {
            match self.0.compare_exchange(0, new, success, failure) {
                Ok(_) => break true,
                Err(value) => {
                    if let Some(lock_owner) = NonZeroU32::new(value as u32)
                        && lock_owner == thread_id
                    {
                        break true;
                    }
                    if value == FUTEX_WAITERS {
                        match self
                            .0
                            .compare_exchange(FUTEX_WAITERS, new, success, failure)
                        {
                            Ok(_) => break true,
                            Err(value) => {
                                if value != 0 {
                                    break false;
                                }
                            }
                        }
                    } else {
                        break false;
                    }
                }
            }
        }
    }

    unsafe fn unlock(&self) {
        // self.0.store(0, Ordering::Release);
        if self
            .0
            .compare_exchange(
                syscall_get_thread_id().thread_id.get().into(),
                0,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_err()
        {
            unsafe { syscall::<SyscallFutexUnlock>(&(&self.0 as *const _ as u64)).unwrap() };
        }
    }
}

pub type BlockingLock<T> = lock_api::Mutex<RawBlockingLock, T>;
pub type BlockingLockGuard<'a, T> = lock_api::MutexGuard<'a, RawBlockingLock, T>;
