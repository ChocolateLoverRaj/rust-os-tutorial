#![no_std]

mod logger;

use log::info;

pub fn start() {
    logger::init();
    info!("Hello from Rust kernel");
}
