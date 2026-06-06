//! Port-mapped I/O (`in`/`out`) — **X86-ONLY**.
//!
//! O x86 tem um espaço de endereçamento separado de 64K "portas", acessível só
//! pelas instruções `in`/`out`. ARM e RISC-V não têm isso (usam só MMIO) — por
//! isso este módulo vive sob `arch/x86_64/` e nunca subirá para o código genérico.
//!
//! É o lar do acesso a portas usado a partir da Fase 2 do `vga-roadmap.md`
//! (cursor de hardware, scroll por hardware, console serial). Ainda não usado.

#![allow(dead_code)]

use ::x86_64::instructions::port::Port;

/// Escreve um byte numa porta de I/O (instrução `out`).
///
/// # Safety
/// Acesso direto a hardware: a porta precisa existir e a escrita pode ter
/// efeitos colaterais arbitrários no dispositivo.
pub unsafe fn outb(port: u16, value: u8) {
    let mut port = Port::new(port);
    unsafe { port.write(value) };
}

/// Lê um byte de uma porta de I/O (instrução `in`).
///
/// # Safety
/// Ver [`outb`]: ler de uma porta pode ter efeitos colaterais no dispositivo.
pub unsafe fn inb(port: u16) -> u8 {
    let mut port = Port::new(port);
    unsafe { port.read() }
}
