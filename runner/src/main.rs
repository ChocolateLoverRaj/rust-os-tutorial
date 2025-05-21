use std::{
    env,
    process::{self, Command},
};

fn main() {
    let iso = env::var("ISO").unwrap();
    println!("ISO path: {:?}", iso);
    let ovmf = env::var("OVMF_PATH").unwrap();

    // Qemu runs our OS in a virtual
    let mut qemu = Command::new("qemu-system-x86_64");

    // Specify the path to the ISO
    qemu.arg("-cdrom");
    qemu.arg(env!("ISO"));
    // For UEFI on qemu, the path to OVMF.fd is needed
    qemu.arg("-bios").arg(ovmf);

    // Pass any args to qemu
    env::args().skip(1).for_each(|arg| {
        qemu.arg(arg);
    });
    let exit_status = qemu.status().unwrap();
    process::exit(exit_status.code().unwrap_or(1));
}
