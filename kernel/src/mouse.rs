use ps2_mouse::MousePacketParser;

pub static MOUSE: spin::Mutex<Option<MousePacketParser>> = spin::Mutex::new(None);

fn try_init() -> Result<(), ()> {
    let mut ps2 = unsafe { ps2::Controller::new() };
    ps2.enable_mouse().map_err(|_| ())?;
    ps2.mouse().set_defaults().map_err(|_| ())?;
    ps2.mouse().enable_data_reporting().map_err(|_| ())?;
    Ok(())
}

/// Interrupts should be disabled while calling this function.
pub fn init() {
    let mut mouse = MOUSE.lock();
    match try_init() {
        Ok(()) => {
            *mouse = Some(Default::default());
            log::debug!("PS/2 mouse initialiezd");
        }
        Err(()) => {
            *mouse = None;
            log::debug!("Failed ot initialize PS/2 mouse (probably doesn't exist)");
        }
    }
}
