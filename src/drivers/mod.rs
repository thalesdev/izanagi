//! Drivers — "tudo é driver", no estilo do Linux. Tudo que fala com hardware
//! vive aqui; o único núcleo genérico é o `kernel/` (a trait `Console` + printk).

pub mod vga;
pub mod video;
