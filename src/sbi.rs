use core::{arch::asm, fmt::Write};

use portable_atomic::AtomicBool;
use sbi_spec::{
    base::{EID_BASE, PROBE_EXTENSION},
    dbcn::EID_DBCN,
};

use crate::sbi::console::{Dbcn, Unknown};

pub struct SbiRet {
    pub a0: usize,
    pub a1: usize,
}

pub unsafe fn sbi_call(
    mut arg0: usize,
    mut arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
    fid: usize,
    eid: usize,
) -> SbiRet {
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") arg0,
            inlateout("a1") arg1,
            in("a2") arg2,
            in("a3") arg3,
            in("a4") arg4,
            in("a5") arg5,
            in("a6") fid,
            in("a7") eid,
        );
    }
    SbiRet { a0: arg0, a1: arg1 }
}

pub fn probe_extension(extension_id: u64) -> Result<bool, usize> {
    let (arg0, arg1) = if size_of::<usize>() == size_of::<u32>() {
        (extension_id as usize, (extension_id >> u32::BITS) as usize)
    } else {
        (extension_id as usize, 0)
    };
    let SbiRet { a0, a1: _ } =
        unsafe { sbi_call(arg0, arg1, 0, 0, 0, 0, PROBE_EXTENSION, EID_BASE) };
    match a0 {
        0 => Ok(false),
        1 => Ok(true),
        n => Err(n),
    }
}

static TOOK_CONSOLE: AtomicBool = AtomicBool::new(false);
pub struct Console<Mode> {
    mode: Mode,
}

mod console {
    pub struct Unknown;
    pub struct Dbcn;
}

impl Console<Unknown> {
    pub fn take() -> Option<Self> {
        if !TOOK_CONSOLE.swap(true, core::sync::atomic::Ordering::Relaxed) {
            Some(Self { mode: Unknown })
        } else {
            None
        }
    }

    pub fn try_into_dbcn(self) -> Result<Console<Dbcn>, Self> {
        if probe_extension(EID_DBCN as u64).is_ok_and(|supported| supported) {
            Ok(Console { mode: Dbcn })
        } else {
            Err(self)
        }
    }
}

impl Write for Console<Dbcn> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let SbiRet { a0, a1 } = unsafe {
            sbi_call(s.len(), s.as_ptr().addr(), 0, 0, 0, arg5, fid, eid)
        }
    }
}
