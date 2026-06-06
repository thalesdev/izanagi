//! Núcleo genérico do kernel — código **agnóstico de hardware** (o "policy").
//! Nada aqui sabe em qual arquitetura ou em qual driver está rodando.

pub mod printk;
