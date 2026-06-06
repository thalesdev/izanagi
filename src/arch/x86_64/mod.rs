//! Suporte à arquitetura x86_64.

pub mod boot;
pub mod port;

/// Inicialização *early* específica da arquitetura, chamada pelo `_start` antes
/// do `kernel_main`. Por ora não faz nada — é o ponto de entrada futuro para
/// GDT, IDT, paging e habilitação de interrupções.
pub fn init() {}

/// Para a CPU permanentemente. Usado no fim do `kernel_main` e no panic handler.
///
/// `hlt` num loop em vez de um `loop {}` "ocupado": deixa a CPU dormir até a
/// próxima interrupção, gastando bem menos energia (e ciclos no QEMU).
pub fn halt() -> ! {
    loop {
        ::x86_64::instructions::hlt();
    }
}
