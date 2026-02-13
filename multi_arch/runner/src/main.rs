use std::{env::current_dir, path::Path, process::Stdio};

use bytes::{BufMut, BytesMut, buf};
use defmt_decoder::{
    Frame, Locations, Table,
    log::{
        format::{Formatter, FormatterConfig, HostFormatter},
        is_defmt_frame,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, stdout},
    process,
};
use tokio_stream::StreamExt;
use tokio_util::codec::{AnyDelimiterCodec, FramedRead};

#[tokio::main]
async fn main() {
    let kernel_dir = current_dir().unwrap().parent().unwrap().join("kernel");
    let output = process::Command::new("cargo")
        .args(["build", "--target", "riscv32imc-unknown-none-elf"])
        .current_dir(&kernel_dir)
        .output()
        .await
        .unwrap();
    if !output.status.success() {
        panic!("build error: {:#?}", output);
    }
    let kernel_path = kernel_dir.join("target/riscv32imc-unknown-none-elf/debug/kernel");
    let elf = tokio::fs::read(&kernel_path).await.unwrap();
    let table = Table::parse(&elf).unwrap().unwrap();
    let locs = table.get_locations(&elf).ok();
    let mut c = process::Command::new("qemu-system-riscv32")
        .args([
            "--machine",
            "virt",
            "--nographic",
            "--serial",
            "mon:stdio",
            "--no-reboot",
            "--kernel",
            kernel_path.to_str().unwrap(),
        ])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let child_stdout = c.stdout.take().unwrap();
    let mut frames = FramedRead::new(
        child_stdout,
        AnyDelimiterCodec::new_with_max_length(vec![0], vec![], 1024 * 1024 * 1024),
    );
    let mut first_frame = frames.next().await.unwrap().unwrap();
    stdout().write_all_buf(&mut first_frame).await.unwrap();
    defmt_decoder::log::init_logger(
        Formatter::new(FormatterConfig::default()),
        HostFormatter::new(FormatterConfig::default()),
        defmt_decoder::log::DefmtLoggerType::Stdout,
        is_defmt_frame,
    );
    while let Some(frame) = frames.next().await {
        let frame = frame.unwrap();
        let mut frame = BytesMut::from(frame);
        frame.reserve(frame.len() + 1);
        frame.put_u8(0);
        match table.decode(&frame) {
            Ok((frame, _)) => forward_to_logger(
                &frame,
                location_info(&locs, &frame, &current_dir().unwrap()),
            ),
            Err(e) => {
                println!("error");
            }
        };
    }
    // stdout();
    // let mut buffer = {
    //     let mut buffer = bytes::BytesMut::with_capacity(1024 * 1024);
    //     loop {
    //         let read_count = stdout.read_buf(&mut buffer).await.unwrap();
    //         if read_count == 0 {
    //             break None;
    //         }
    //         if let Some(position) = buffer.iter().position(|&byte| byte == 0) {
    //             let leftover = buffer.split_off(position);
    //             tokio::io::stdout()
    //                 .write_all_buf(&mut buffer)
    //                 .await
    //                 .unwrap();
    //             break Some(leftover);
    //         } else {
    //             tokio::io::stdout()
    //                 .write_all_buf(&mut buffer)
    //                 .await
    //                 .unwrap();
    //         }
    //     }
    // };
    // if let Some(mut buffer) = buffer {
    //     loop {
    //         loop {
    //             if let Some(zero_index) = buffer.iter().position(|byte| *byte == 0) {
    //             } else {
    //             }
    //         }
    //         stdout.read_buf(&mut buffer).await.unwrap();
    //     }
    // }
}

type LocationInfo = (Option<String>, Option<u32>, Option<String>);
fn location_info(locs: &Option<Locations>, frame: &Frame, current_dir: &Path) -> LocationInfo {
    let (mut file, mut line, mut mod_path) = (None, None, None);

    let loc = locs.as_ref().map(|locs| locs.get(&frame.index()));

    if let Some(Some(loc)) = loc {
        // try to get the relative path, else the full one
        let path = loc.file.strip_prefix(current_dir).unwrap_or(&loc.file);

        file = Some(path.display().to_string());
        line = Some(loc.line as u32);
        mod_path = Some(loc.module.clone());
    }

    (file, line, mod_path)
}

fn forward_to_logger(frame: &Frame, location_info: LocationInfo) {
    let (file, line, mod_path) = location_info;
    defmt_decoder::log::log_defmt(frame, file.as_deref(), line, mod_path.as_deref());
}
