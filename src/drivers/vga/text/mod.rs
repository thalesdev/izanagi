//! VGA modo texto (modo 3 do BIOS: 80×25, 16 cores) — o "vgacon".
//!
//! Escreve células diretamente no buffer MMIO em `0xB8000`. É **X86-PC-only**:
//! esse endereço e esse formato de célula são herança da IBM PC e não existem em
//! ARM/RISC-V. O que atravessa arquiteturas é a trait `Console` que o [`VgaText`]
//! implementa, não este driver.

mod buffer;
mod color;
mod writer;

#[allow(unused_imports)]
pub use buffer::{Buffer, ScreenChar, BUFFER_HEIGHT, BUFFER_WIDTH};
#[allow(unused_imports)]
pub use color::{Color, ColorCode};
#[allow(unused_imports)]
pub use writer::{VgaText, Writer, VGA, WRITER};
