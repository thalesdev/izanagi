use lazy_static::lazy_static;
use spin::Mutex;

use crate::kernel::printk::Console;
use super::buffer::{Buffer, ScreenChar, BUFFER_HEIGHT, BUFFER_WIDTH};
use super::color::{Color, ColorCode};

/// Estado mutável do console de texto: onde estamos escrevendo, com qual cor, e
/// uma referência ao buffer MMIO. Sempre acessado através do [`WRITER`] (atrás
/// de um `Mutex`), nunca diretamente.
pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}

#[allow(dead_code)]
impl Writer {
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                _ => self.write_byte(0xfe),
            }
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let char = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(char);
            }
        }

        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }

    fn clear_screen(&mut self) {
        for row in 0..BUFFER_HEIGHT {
            self.clear_row(row);
        }
        self.column_position = 0;
    }
}

lazy_static! {
    /// O estado global do console VGA texto. `lazy_static` porque a inicialização
    /// desreferencia o ponteiro cru `0xb8000`, o que não é possível em contexto
    /// `const`/`static` — é adiada para o runtime, na primeira utilização.
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

/// O backend de console VGA texto (o "vgacon").
///
/// É *zero-sized*: todo o estado vive no [`WRITER`] global. Por isso pode ser um
/// `static` simples e `&VGA` vira um `&'static dyn Console` registrável no printk
/// sem nenhuma dança de lifetime.
pub struct VgaText;

/// Instância estática a ser registrada em `kernel::printk::register_console`.
pub static VGA: VgaText = VgaText;

impl Console for VgaText {
    fn write_str(&self, s: &str) {
        WRITER.lock().write_string(s);
    }

    fn clear(&self) {
        WRITER.lock().clear_screen();
    }
}
