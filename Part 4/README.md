# Logger
As we expand our kernel, it will be very useful to be able to log messages. The `log` crate provides macros similar to `println`, and works on `no_std` because you have to write your own log function implementation. Add this to the kernel deps:
```toml
log = "0.4.27"
```
Then create a new file called `logger.rs`:
```rs
use log::Log;
use uart_16550::SerialPort;

struct KernelLogger {
    serial_port: SerialPort,
}

impl Log for KernelLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        todo!()
    }

    fn log(&self, record: &log::Record) {
        todo!()
    }

    fn flush(&self) {
        todo!()
    }
}
```
We have to implement the `log` function, but we are only given an `&self` function. We need to put `SerialPort` in something which has interior mutability. We can use a [mutex](https://en.wikipedia.org/wiki/Lock_(computer_science)) to achieve this. Normally, we would use the `Mutex` from `std`. In `no_std`, we can use the `spinning_top` crate for a simple mutex. It works by continuously checking in a loop if the mutex is available.
```toml
spinning_top = "0.3.0"
```
```rs
struct KernelLogger {
    serial_port: Spinlock<SerialPort>,
}
```
and in the `log` function, we can do
```rs
let mut serial_port = self.serial_port.try_lock().unwrap();
let level = record.level();
let args = record.args();
writeln!(serial_port, "{:5} {}", level, args).unwrap();
```
Mutexes can feel like an easy fix to the issue of having immutable variables, but whenever we use mutexes we have to keep in mind the possibility of a [deadlock](https://en.wikipedia.org/wiki/Deadlock_(computer_science)). This is why we use `.try_lock().unwrap()` instead of `.lock()`, so that we panic by default if the lock is busy, so we know for sure there won't be a deadlock.

Now let's have a global variable for our logger and a function to initialize our logger:
```rs
static LOGGER: KernelLogger = KernelLogger {
    serial_port: Spinlock::new(unsafe { SerialPort::new(0x3F8) }),
};

pub fn init() -> Result<(), log::SetLoggerError> {
    LOGGER.serial_port.try_lock().unwrap().init();
    log::set_max_level(LevelFilter::max());
    log::set_logger(&LOGGER)
}
```
Note that the `log` crate requires us to set a level filter, which lets us choose to only log messages with a certain importance. For example, we can set the level filter to only log warn and error messages, and not log info, debug, or trace messages. You can try it out by setting the max level to `LevelFilter::Warn`. Then you will not see any messages from `log::info`.

Now we can log from `main.rs` like this:
```rs
logger::init().unwrap();
log::info!("Hello World!");
```
And you should see
```
INFO  Hello World!
```

# Make it colorful!
Your terminal supports colors, and when we redirect COM1 to your terminal, we can print colors from our kernel. So let's do it, because it's cool and it improves readability. We'll use the `owo_colors` crate to handle coloring with [ANSI escape codes](https://en.wikipedia.org/wiki/ANSI_escape_code#Colors).
```toml
owo-colors = "4.2.1"
```
Then in our logger:
```rs
use owo_colors::OwoColorize;
```
```rs
let level: &dyn Display = match level {
    log::Level::Error => &level.bright_red(),
    log::Level::Warn => &level.bright_yellow(),
    log::Level::Info => &level.bright_blue(),
    log::Level::Debug => &level.bright_cyan(),
    log::Level::Trace => &level.bright_magenta(),
};
```
And now the log level text will be colorful!

![Picture of a "INFO  Hello World!" message with the "INFO" text being blue](./Colorful_Log_Message.png)

Now that we have a logger, let's update our panic handler:
```rs
#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("{}", info);
    hlt_loop();
}
```
To check our panic handler, let's change `hlt_loop` to `todo!()` in our entry function. Now we should see
```
INFO  Hello World!
ERROR panicked at kernel/src/main.rs:34:5:
not yet implemented
```
