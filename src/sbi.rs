use core::{arch::asm, fmt::Write};

use portable_atomic::AtomicBool;
use sbi_spec::{
    base::{EID_BASE, PROBE_EXTENSION},
    binary::RET_SUCCESS,
    dbcn::{CONSOLE_WRITE, EID_DBCN},
    legacy::{LEGACY_CONSOLE_PUTCHAR, LEGACY_SHUTDOWN},
};

pub struct SbiRet {
    pub a0: usize,
    pub a1: usize,
}

pub unsafe fn legacy_sbi_call(
    mut arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
    eid: usize,
) -> usize {
    unsafe {
        asm!(
            "ecall",
            inlateout("a0") arg0,
            in("a1") arg1,
            in("a2") arg2,
            in("a3") arg3,
            in("a4") arg4,
            in("a5") arg5,
            in("a7") eid,
        );
    };
    arg0
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

pub fn probe_extension(extension_id: usize) -> Result<bool, usize> {
    let SbiRet { a0, a1: _ } =
        unsafe { sbi_call(extension_id, 0, 0, 0, 0, 0, PROBE_EXTENSION, EID_BASE) };
    match a0 {
        0 => Ok(false),
        1 => Ok(true),
        n => Err(n),
    }
}

static TOOK_CONSOLE: AtomicBool = AtomicBool::new(false);

enum ConsoleMethod {
    LegacyConsole,
    Dbcn,
}

pub struct Console {
    method: ConsoleMethod,
}

impl Console {
    pub fn take() -> Option<Self> {
        if !TOOK_CONSOLE.swap(true, core::sync::atomic::Ordering::Relaxed) {
            Some(Self {
                method: if probe_extension(EID_DBCN).is_ok_and(|supported| supported) {
                    ConsoleMethod::Dbcn
                } else {
                    ConsoleMethod::LegacyConsole
                },
            })
        } else {
            None
        }
    }
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        match self.method {
            ConsoleMethod::LegacyConsole => {
                for &char in s.as_bytes() {
                    let result = unsafe {
                        legacy_sbi_call(char as usize, 0, 0, 0, 0, 0, LEGACY_CONSOLE_PUTCHAR)
                    };
                    if result != 0 {
                        Err(core::fmt::Error)?;
                    }
                }
            }
            ConsoleMethod::Dbcn => {
                let mut bytes_written = 0;
                while bytes_written < s.len() {
                    let SbiRet { a0, a1 } = unsafe {
                        sbi_call(
                            s.len(),
                            s.as_ptr().addr(),
                            0,
                            0,
                            0,
                            0,
                            CONSOLE_WRITE,
                            EID_DBCN,
                        )
                    };
                    if a1 != RET_SUCCESS {
                        Err(core::fmt::Error)?;
                    }
                    bytes_written += a0;
                    // bytes_written += self.write(s.as_bytes()).map_err(|e| core::fmt::Error)?;
                }
            }
        }
        Ok(())
    }
}

pub fn shutdown() -> ! {
    unsafe { legacy_sbi_call(0, 0, 0, 0, 0, 0, LEGACY_SHUTDOWN) };
    unreachable!()
}
