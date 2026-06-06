//! Camada por-arquitetura — o "facade" no estilo dos headers `asm/` do Linux.
//!
//! Cada arquitetura suportada vive num submódulo e expõe **a mesma superfície**
//! (`init`, `halt`, port/mmio, ...). Este módulo seleciona a arch ativa via
//! `#[cfg(target_arch)]` e re-exporta, de modo que o kernel genérico chame
//! apenas `arch::init()` / `arch::halt()` sem saber em qual CPU está rodando.
//!
//! Hoje só existe `x86_64`. Adicionar `riscv64`/`aarch64` no futuro é aditivo:
//! cria-se o submódulo irmão com a mesma superfície e mais um par de `#[cfg]`.

#[cfg(target_arch = "x86_64")]
pub mod x86_64;
// `self::` é obrigatório: um `use x86_64::...` "pelado" apontaria para o CRATE
// externo `x86_64` (regra de path ancorado do edition 2018+), não para o
// submódulo local de mesmo nome.
#[cfg(target_arch = "x86_64")]
pub use self::x86_64::{halt, init};
