use crate::UART_BASE;

pub struct Uart {}
impl core::fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.bytes() {
            unsafe {
                UART_BASE.write_volatile(c);
            }
        }

        Ok(())
    }
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") };
    ($($t:tt)*) => { $crate::uart::print_fmt(format_args!("{}\n", format_args!($($t)*))) };
}

pub fn print_fmt(args: core::fmt::Arguments) {
    <Uart as core::fmt::Write>::write_fmt(&mut Uart {}, args).unwrap();
}
