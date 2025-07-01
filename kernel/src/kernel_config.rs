use core::str::Utf8Error;

use log::LevelFilter;
use ron::de::SpannedError;
use serde::{Deserialize, Serialize};
use spin::Once;
use thiserror::Error;

use crate::{limine_requests::MODULE_REQUEST, user_mode_program_path::KERNEL_CONFIG_PATH};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[non_exhaustive]
pub struct KernelConfig {
    pub log_serial: LevelFilter,
    pub log_screen: LevelFilter,
    pub log_sample_messages: bool,
}

impl KernelConfig {
    pub const DEFAULT: Self = Self {
        log_serial: LevelFilter::Debug,
        log_screen: LevelFilter::Warn,
        log_sample_messages: true,
    };
}

#[derive(Debug, Error)]
pub enum GetConfigError {
    #[error("No config file provided")]
    NoConfigModule,
    #[error("The config file was not valid UTF-8")]
    InvalidStr(Utf8Error),
    #[error("The config file was not valid RON")]
    ParseError(SpannedError),
}

fn try_get_config() -> Result<KernelConfig, GetConfigError> {
    let module = MODULE_REQUEST
        .get_response()
        .unwrap()
        .modules()
        .iter()
        .find(|module| module.path() == KERNEL_CONFIG_PATH)
        .ok_or(GetConfigError::NoConfigModule)?;
    let data = module.addr();
    let len = module.size() as usize;
    // Safety: Limine ensures the data is valid and we only access it immutably
    let module = unsafe { core::slice::from_raw_parts(data, len) };
    let kernel_config = str::from_utf8(module).map_err(GetConfigError::InvalidStr)?;
    let kernel_config = ron::from_str(kernel_config).map_err(GetConfigError::ParseError)?;
    Ok(kernel_config)
}

static KERNEL_CONFIG: Once<Result<KernelConfig, GetConfigError>> = Once::new();

/// Returns the specified kernel config if succesfully parsed.
/// Returns the default config if the config was not yet parsed
/// or if there was an error parsing the config.
pub fn get_or_default() -> &'static KernelConfig {
    if let Some(result) = KERNEL_CONFIG.get() {
        match result {
            Ok(kernel_config) => kernel_config,
            Err(_) => &KernelConfig::DEFAULT,
        }
    } else {
        &KernelConfig::DEFAULT
    }
}

/// Call this function after the logger and global allocator are initialized
pub fn init() {
    let result = KERNEL_CONFIG.call_once(try_get_config);
    if let Err(e) = result {
        log::warn!("{e:?}");
    }
    if result
        .as_ref()
        .unwrap_or(&KernelConfig::DEFAULT)
        .log_sample_messages
    {
        for level in log::Level::iter() {
            log::log!(level, "Sample {level} log message");
        }
    }
}
