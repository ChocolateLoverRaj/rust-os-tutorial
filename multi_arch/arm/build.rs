use cfg_if::cfg_if;

fn main() {
    println!("cargo:rustc-link-arg=-Tlinker.ld");
    // On qemu this is 0x10000, but on a real Raspberry Pi Zero this is 0x8000
    // See https://forum.osdev.org/viewtopic.php?p=260378&hilit=raspi+0x8000#p260378,
    // https://forum.osdev.org/viewtopic.php?t=29998
    #[allow(unused)]
    enum TargetBoard {
        HwRaspi,
        QemuRaspi,
        QemuVirt,
    }

    impl TargetBoard {
        fn kernel_start(&self) -> u32 {
            match self {
                Self::HwRaspi => 0x8000,
                Self::QemuRaspi => 0x10000,
                Self::QemuVirt => 0x40010000,
            }
        }
    }

    let target_board = {
        cfg_if! {
            if #[cfg(feature = "hw_raspi")] {
                TargetBoard::HwRaspi
            } else if #[cfg(feature = "qemu_raspi")] {
                TargetBoard::QemuRaspi
            } else if #[cfg(feature = "qemu_virt")] {
                TargetBoard::QemuVirt
            } else {
                compile_error!("no target board selected")
            }
        }
    };

    let kernel_start = target_board.kernel_start();
    println!("cargo:rustc-link-arg=--defsym=KERNEL_START={kernel_start:#X}");
}
