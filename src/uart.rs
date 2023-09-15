use core::fmt::Write;
use crate::stolen_uart::MmioSerialPort;

pub static mut TERM: Option<MmioSerialPort> = None;
static mut X_POS: u8 = 0;
pub fn init_from_mmio(addr: usize) {
    unsafe {
        TERM = Some(MmioSerialPort::new(addr));
        TERM.as_mut().unwrap().init();
        X_POS = 0;
    }
}

struct MmioSerialWithXPos<'a> {
    ser: &'a mut MmioSerialPort,
}

impl Write for MmioSerialWithXPos<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        unsafe {
            for char in s.bytes() {
                if (X_POS >= 80) || (char == b'\n') {
                    X_POS = 0;
                    if char != b'\n' {
                        self.ser.send(b'\r');
                        self.ser.send(b'\n');
                    }
                    self.ser.send(char)
                } else {
                    self.ser.send(char);
                    X_POS += 1
                }
            }
            Ok(())
        }
    }
}

#[macro_export]
macro_rules! print {
    ($($t:tt)*) => { $crate::uart::print_fmt(format_args!($($t)*)) };
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\r\n") };
    ($($t:tt)*) => { $crate::uart::print_fmt(format_args!("{}\r\n", format_args!($($t)*))) };
}

#[macro_export]
macro_rules! readln {
    () => {{
        let mut _lastchar = b'\x00';
        let mut output = alloc::string::String::new();
        let _term = $crate::uart::TERM.as_mut().unwrap();
        while _lastchar != b'\r' {
            _lastchar = _term.receive();
            output.push(_lastchar as char);
            print!("{}", _lastchar as char);
        }
        println!();
        output
    }};
}

pub fn print_fmt(args: core::fmt::Arguments) {
    if let Some(term) = unsafe { TERM.as_mut() } {
        let _ = (MmioSerialWithXPos { ser: term }).write_fmt(args);
    }
}

pub struct UartLogger {}

impl log::Log for UartLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        println!("{}@{}", record.module_path().unwrap_or("?"), record.args())
    }

    fn flush(&self) {}
}
