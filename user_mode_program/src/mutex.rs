use core::sync::atomic::{AtomicU64, Ordering};

use common::{SyscallAquireLock, SyscallReleaseLock, SyscallTryAquireLock};
use lock_api::{GuardNoSend, RawMutex};

use crate::syscalls::syscall;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub struct RawBlockingLock {
    /// `0` - no id set
    ///
    /// `1..` - the id that was set
    id: AtomicU64,
}

impl RawBlockingLock {
    fn get_or_init_id(&self) -> u64 {
        let new_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        assert_ne!(new_id, 0);
        match self
            .id
            .compare_exchange(0, new_id, Ordering::Release, Ordering::Acquire)
        {
            Ok(_) => new_id,
            Err(existing_id) => existing_id,
        }
    }
}

unsafe impl RawMutex for RawBlockingLock {
    const INIT: Self = Self {
        id: AtomicU64::new(0),
    };

    type GuardMarker = GuardNoSend;

    fn lock(&self) {
        let input = self.get_or_init_id();
        unsafe { syscall::<SyscallAquireLock>(&input) }
    }

    fn try_lock(&self) -> bool {
        let input = self.get_or_init_id();
        unsafe { syscall::<SyscallTryAquireLock>(&input) }
    }

    unsafe fn unlock(&self) {
        let input = self.get_or_init_id();
        unsafe { syscall::<SyscallReleaseLock>(&input) }
    }
}

pub type BlockingLock<T> = lock_api::Mutex<RawBlockingLock, T>;
pub type BlockingLockGuard<'a, T> = lock_api::MutexGuard<'a, RawBlockingLock, T>;
