//! Subsistema de vídeo agnóstico de hardware — espelha `drivers/video/` do Linux.
//!
//! Aqui mora o **contrato** ([`Framebuffer`]) — a camada "fb core". Os drivers de
//! hardware o implementam (VGA modos 13h/12h no x86; framebuffers descritos por
//! Device Tree em ARM/RISC-V; GPUs), e o `FbConsole` (Fase B) o consome para
//! desenhar texto em pixels e virar um [`Console`](crate::kernel::printk::Console).
//!
//! Por que isto fica em `drivers/` e não num `video/` no topo? Porque a trait
//! vive na camada de quem a **consome**: `Framebuffer` só é usada dentro de
//! `drivers/`, então não é um subsistema central do kernel. (`Console`, essa sim,
//! é consumida pelo `kernel/printk`, e por isso mora no kernel.)
//!
//! Ainda sem implementações — é o terreno preparado para a Fase B do plano.

#![allow(dead_code)]

/// Um pixel/cor genérico. Conforme o modo do framebuffer, os componentes
/// representam RGB (ex.: framebuffer linear do UEFI) ou são reinterpretados como
/// índice de paleta (ex.: VGA modo 13h, onde só o primeiro byte importa).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8);

/// Saída gráfica baseada em pixels. Implementar isto = plugar um novo modo ou
/// dispositivo de vídeo; o `FbConsole` desenha glifos por cima sem saber qual é.
pub trait Framebuffer {
    /// Dimensões `(largura, altura)` em pixels.
    fn dimensions(&self) -> (usize, usize);
    /// Pinta um pixel. Coordenadas fora dos limites devem ser ignoradas pela impl.
    fn put_pixel(&mut self, x: usize, y: usize, color: Color);
}
