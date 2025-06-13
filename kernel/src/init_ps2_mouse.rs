use spin::Once;

static PS2_MOUSE_EXISTS: Once<bool> = Once::new();

fn try_init() -> Result<(), ()> {
    let mut ps2 = unsafe { ps2::Controller::new() };
    ps2.enable_mouse().map_err(|_| ())?;
    ps2.mouse().set_defaults().map_err(|_| ())?;
    ps2.mouse().enable_data_reporting().map_err(|_| ())?;
    Ok(())
}

/// Trys to initialize the mouse, storing if the mouse exists or not
pub fn init() {
    PS2_MOUSE_EXISTS.call_once(|| try_init().is_ok());
}

pub fn mouse_exists() -> bool {
    *PS2_MOUSE_EXISTS.get().unwrap()
}
