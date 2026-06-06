#![no_std]
#![no_main]

mod arch;
mod drivers;
mod kernel;

use core::panic::PanicInfo;

fn kernel_main() -> ! {
    kernel::printk::register_console(&drivers::vga::VGA);

    println!("Hello World{}", "!");
    panic!("pain games joga amanha!");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    arch::halt()
}
