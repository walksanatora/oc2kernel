pub struct Uart {}
impl core::fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let uart_out = 0x1000_0148 as *mut u8;

        for c in s.bytes() {
            unsafe {
                uart_out.write_volatile(c);
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
