//! Ponto de entrada do kernel no x86_64 (boot legado via `bootloader 0.9`/BIOS).
//!
//! O bootloader carrega o kernel e pula para o símbolo de entrada da ELF, que
//! por convenção do linker é `_start`. Por isso o `#[unsafe(no_mangle)]`: o nome
//! precisa sair intacto para o bootloader encontrá-lo.

/// Símbolo de entrada da ELF — a primeira coisa que roda no kernel.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    crate::arch::init();
    crate::kernel_main();
}
