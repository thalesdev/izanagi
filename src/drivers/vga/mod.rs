//! Driver VGA — **X86-PC-only** (`#[cfg(target_arch = "x86_64")]` no consumidor).
//!
//! - [`text`]: modo texto (modo 3), já implementado — o backend de `Console`.
//! - (Fase B) `modes/`: modos gráficos 13h/12h, que implementarão `Framebuffer`.
//! - (Fase B) `regs.rs`: os grupos de registradores VGA para *modeset* em long mode.

pub mod text;

#[allow(unused_imports)]
pub use text::{VgaText, VGA};
