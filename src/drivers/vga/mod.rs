mod color;
mod buffer;
mod writer;

#[allow(unused_imports)]
pub use color::{Color, ColorCode};
#[allow(unused_imports)]
pub use buffer::{ScreenChar, Buffer, BUFFER_HEIGHT, BUFFER_WIDTH};
#[allow(unused_imports)]
pub use writer::{Writer, WRITER, _print};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::drivers::vga::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
