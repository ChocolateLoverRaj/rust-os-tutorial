use std::{
    env,
    fs::{create_dir_all, remove_file},
    io::{self, ErrorKind},
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Stdio,
};

fn main() {
    // This is the folder where a build script (this file) should place its output
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    // This is the `runner` folder
    let runner_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // This folder contains Limine files such as `BOOTX64.EFI`
    let limine_dir = PathBuf::from(env::var("LIMINE_PATH").unwrap());

    // We will create an ISO file for our OS
    // First we create a folder which will be used to generate the ISO
    // We will use symlinks instead of copying to avoid unnecessary disk space used
    let iso_dir = out_dir.join("iso_root");
    create_dir_all(&iso_dir).unwrap();

    // In the ISO, the config will be at boot/limine/limine.conf
    let boot_dir = iso_dir.join("boot");
    create_dir_all(&boot_dir).unwrap();
    let out_limine_dir = boot_dir.join("limine");
    create_dir_all(&out_limine_dir).unwrap();
    let limine_conf = out_limine_dir.join("limine.conf");
    ensure_symlink(runner_dir.join("limine.conf"), limine_conf).unwrap();

    // Copy files from the Limine packaeg into `boot/limine`
    for path in [
        "limine-bios.sys",
        "limine-bios-cd.bin",
        "limine-uefi-cd.bin",
    ] {
        let from = limine_dir.join(path);
        let to = out_limine_dir.join(path);
        ensure_symlink(from, to).unwrap();
    }

    // EFI/BOOT/BOOTX64.EFI is the executable loaded by UEFI firmware
    // We will also copy BOOTIA32.EFI because xorisso will complain if it's not there
    let efi_boot_dir = iso_dir.join("EFI/BOOT");
    create_dir_all(&efi_boot_dir).unwrap();
    for efi_file in ["BOOTX64.EFI", "BOOTIA32.EFI"] {
        ensure_symlink(limine_dir.join(efi_file), efi_boot_dir.join(efi_file)).unwrap();
    }

    // We'll call the output iso `os.iso`
    let output_iso = out_dir.join("os.iso");
    // This command creates an ISO file from our `iso_root` folder.
    // Symlinks will be read (the contents will be copied into the ISO file)
    let status = std::process::Command::new("xorriso")
        .arg("-as")
        .arg("mkisofs")
        .arg("--follow-links")
        .arg("-b")
        .arg(
            out_limine_dir
                .join("limine-bios-cd.bin")
                .strip_prefix(&iso_dir)
                .unwrap(),
        )
        .arg("-no-emul-boot")
        .arg("-boot-load-size")
        .arg("4")
        .arg("-boot-info-table")
        .arg("--efi-boot")
        .arg(
            out_limine_dir
                .join("limine-uefi-cd.bin")
                .strip_prefix(&iso_dir)
                .unwrap(),
        )
        .arg("-efi-boot-part")
        .arg("--efi-boot-image")
        .arg("--protective-msdos-label")
        .arg(iso_dir)
        .arg("-o")
        .arg(&output_iso)
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit())
        .status()
        .unwrap();
    assert!(status.success());

    // This is needed to create a hybrid ISO that boots on both BIOS and UEFI. See https://github.com/limine-bootloader/limine/blob/v9.x/USAGE.md#biosuefi-hybrid-iso-creation
    let status = std::process::Command::new("limine")
        .arg("bios-install")
        .arg(&output_iso)
        .stderr(Stdio::inherit())
        .stdout(Stdio::inherit())
        .status()
        .unwrap();
    assert!(status.success());

    // This will let `main.rs` see the path of the ISO we created
    println!("cargo:rustc-env=ISO={}", output_iso.display());
}

pub fn ensure_symlink<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> io::Result<()> {
    match remove_file(&link) {
        Ok(()) => Ok(()),
        Err(error) => match error.kind() {
            ErrorKind::NotFound => Ok(()),
            _ => Err(error),
        },
    }?;
    symlink(original, link)?;
    Ok(())
}
