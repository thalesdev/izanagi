//! Subsistema de log/console — o "printk" do izanagi.
//!
//! Inversão de dependência no estilo do Linux: este subsistema **não conhece
//! driver nenhum**. Ele expõe a trait [`Console`] e mantém uma lista de consoles
//! registrados; cada driver (VGA, serial, framebuffer...) se registra
//! implementando a trait, e `_print` itera sobre todos. Trocar/empilhar backends
//! de saída nunca toca neste arquivo.
//!
//! É o equivalente ao trio do Linux `include/linux/console.h` (o contrato) +
//! `kernel/printk/printk.c` (a lista e o despacho).

use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;

/// Um sink de **saída** de texto registrável — o destino do printk.
///
/// Espelha o `struct console` do Linux (`include/linux/console.h`): é
/// **output-only de propósito**. O printk só cospe bytes, nunca lê. O console
/// *interativo* (input do teclado, echo, edição de linha, stdin, sessões) NÃO é
/// isto — é uma camada bem maior e separada, o subsistema TTY/VT, que virá por
/// cima no futuro (Fase 8 do `vga-roadmap.md`), e não como métodos extras aqui.
/// "Console" aqui = sink de saída; o terminal interativo completo terá outro nome.
///
/// Repare no `&self` + `Sync`: os métodos NÃO pedem `&mut self`. Cada
/// implementação guarda seu estado mutável atrás da própria trava (interior
/// mutability). Isso nos deixa guardar `&'static dyn Console` compartilhado, sem
/// `&'static mut` (que seria um pesadelo de borrow-checker) e sem heap.
pub trait Console: Sync {
    /// Escreve uma fatia de texto no dispositivo.
    fn write_str(&self, s: &str);
    /// Limpa a tela/saída.
    fn clear(&self);
}

/// Ponte entre `core::fmt` e [`Console`].
///
/// `fmt::Write` (que `write_fmt`/`format_args!` precisam) exige `&mut self`, mas
/// `Console::write_str` é `&self`. Este adapter de vida curta carrega um
/// `&dyn Console` e satisfaz `fmt::Write` reencaminhando para ele — assim
/// reaproveitamos toda a maquinaria de formatação sem alocar nada.
struct ConsoleWriter<'a>(&'a dyn Console);

impl fmt::Write for ConsoleWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write_str(s);
        Ok(())
    }
}

/// Quantos consoles podem estar registrados ao mesmo tempo. Tabela de tamanho
/// fixo de propósito: ainda não temos um alocador, então nada de `Vec`.
const MAX_CONSOLES: usize = 4;

lazy_static! {
    /// Os consoles registrados. `None` = slot livre.
    static ref CONSOLES: Mutex<[Option<&'static dyn Console>; MAX_CONSOLES]> =
        Mutex::new([None; MAX_CONSOLES]);
}

/// Pluga um console na lista de saída. Chamado pelos drivers durante o boot.
/// Se a tabela já estiver cheia, o console é silenciosamente ignorado.
pub fn register_console(console: &'static dyn Console) {
    let mut consoles = CONSOLES.lock();
    for slot in consoles.iter_mut() {
        if slot.is_none() {
            *slot = Some(console);
            return;
        }
    }
}

/// Despacha argumentos formatados para todos os consoles registrados.
/// Não chame diretamente — use as macros [`print!`]/[`println!`].
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    for console in CONSOLES.lock().iter().flatten() {
        let _ = ConsoleWriter(*console).write_fmt(args);
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::kernel::printk::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
