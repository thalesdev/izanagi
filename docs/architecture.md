# Arquitetura do `izanagi` — o caminho das pedras

> Companheiro do [vga-roadmap.md](vga-roadmap.md). Enquanto aquele guia ensina o
> driver de texto fase a fase, **este** descreve a *forma do projeto*: como os
> diretórios se dividem, onde moram as traits, por que, e como o Linux resolve os
> mesmos problemas. Leia quando estiver na dúvida sobre "onde isso deveria ficar?".

## Os dois princípios que decidem tudo

### 1. VGA não é portável — as *traits* são

VGA (`0xB8000`, port I/O `in`/`out`, modos 12h/13h) é **x86-PC e ponto final**.
Não existe em ARM nem RISC-V: essas arquiteturas não têm espaço de portas, e as
máquinas `virt` do QEMU não têm hardware VGA. Logo `drivers/vga/` é, e sempre
será, um driver `x86`-only.

O que atravessa arquiteturas é a **abstração**: a trait
[`Console`](../src/kernel/printk.rs) (saída de texto) e a trait
[`Framebuffer`](../src/drivers/video/mod.rs) (saída gráfica). O VGA é só *uma*
implementação delas no x86; amanhã, no ARM/RISC-V, a saída virá de um framebuffer
descrito pelo firmware ou de uma UART — outras implementações das **mesmas** traits.

### 2. Uma trait mora na camada de quem a *consome*

Não de quem a implementa. É a regra que responde "onde isso fica?":

| Trait | Quem consome | Onde mora | Análogo no Linux |
|---|---|---|---|
| `Console` | `kernel/printk` (código genérico do kernel) | `src/kernel/printk.rs` | `include/linux/console.h` |
| `Framebuffer` | `fbcon` (um driver), implementada por drivers de hw | `src/drivers/video/mod.rs` | `include/linux/fb.h` → usada em `drivers/video/` |

Por isso **não existe** `src/video/` no topo: framebuffer é um contrato *interno
da camada de driver*, não um subsistema central. O Linux concorda — o topo dele é
`arch/ kernel/ mm/ fs/ drivers/ ...`, e o vídeo vive em `drivers/video/`.

## A árvore

```
src/
├── main.rs              kernel_main() agnóstico + panic handler
├── arch/                ── código POR-ARQUITETURA (o "asm/" do Linux) ──
│   ├── mod.rs           facade: #[cfg(target_arch)] re-exporta a arch ativa
│   └── x86_64/
│       ├── mod.rs       init(), halt()
│       ├── boot.rs      _start  (entry do bootloader 0.9)
│       └── port.rs      port I/O in/out — X86-ONLY
├── kernel/              ── núcleo GENÉRICO, agnóstico de hardware ("policy") ──
│   ├── mod.rs
│   └── printk.rs        trait Console + register_console + _print + print!/println!
└── drivers/             ── "tudo é driver" (Linux) ──
    ├── vga/             X86-PC-only
    │   ├── mod.rs
    │   └── text/        modo 3 (80×25): VgaText → impl Console  ("vgacon")
    │       ├── color.rs · buffer.rs · writer.rs
    │       ├── (FASE B) modes/  mode13h.rs, mode12h.rs → impl Framebuffer
    │       └── (FASE B) regs.rs grupos de registradores p/ modeset
    └── video/           subsistema de vídeo AGNÓSTICO  (Linux: drivers/video/)
        ├── mod.rs       trait Framebuffer + Color  (o "fb core")
        ├── (FASE B) fbcon.rs  FbConsole<F: Framebuffer> → impl Console
        └── (FASE B) font.rs   fonte bitmap 8×16
```

As três camadas e a regra de dependência entre elas:

```
   main.rs / kernel/          chama → arch::init(), arch::halt()
   (genérico)                 chama → println!  → printk → trait Console
        │                                                      ▲
        │                                            implementam │
        ▼                                                       │
   drivers/  ──────────────────────────────────────────────────┘
   (VgaText, futuros backends)   dependem de: kernel (traits) e arch (port I/O)
        │
        ▼
   arch/x86_64/  (port I/O, boot)   não depende de ninguém acima
```

Regra: **genérico nunca depende de driver nem de arch concreta**; depende só de
*traits*. Drivers dependem das traits (kernel) e das primitivas (arch). É o que
permite trocar/empilhar backends e, no futuro, trocar de arquitetura.

## A inversão de dependência (`printk` → `Console`)

O coração do projeto. Antes, `println!` conhecia o VGA direto. Agora:

```
   println!                         o printk NÃO conhece driver nenhum.
      │                             Os drivers é que se registram nele.
      ▼
   printk::_print()  ──itera──►  [ &dyn Console, &dyn Console, ... ]
                                       │            │
                                       │            └─ (futuro) Serial, FbConsole...
                                       └─ VgaText  (registrado em kernel_main)
```

Mecânica em [printk.rs](../src/kernel/printk.rs), com duas decisões que valem
entender:

- **`trait Console` usa `&self` + `Sync`**, não `&mut self`. Cada backend guarda
  o estado mutável atrás da própria trava (o `VgaText` delega para um
  `Mutex<Writer>` global). Assim podemos guardar `&'static dyn Console`
  compartilhado, sem a dor de `&'static mut` e sem heap.
- **`ConsoleWriter`** é um adapter de vida curta que liga `core::fmt` à trait:
  `write_fmt`/`format_args!` exigem `fmt::Write` (que pede `&mut self`), mas
  `Console::write_str` é `&self`. O adapter satisfaz `fmt::Write` reencaminhando —
  reaproveitando toda a formatação sem alocar.

A lista de consoles é uma **tabela de tamanho fixo** (`[Option<…>; 4]`), não um
`Vec`: ainda não temos alocador. Quando tivermos heap, vira dinâmica.

> **`Console` é output-only, de propósito.** É o equivalente ao `struct console`
> do Linux (o destino do printk), **não** o terminal interativo. A palavra
> "console" é sobrecarregada: o conceito maior — input de teclado, echo, edição de
> linha, stdin, sessões — é uma camada **separada**, o subsistema **TTY/VT**
> (`drivers/tty/` no Linux; `vc_data` por terminal). Ele virá por cima depois
> (Fase 8 do `vga-roadmap.md`), com outro nome (`Tty`/`Vt`), sem colisão — assim
> como o Linux tem `struct console` *e* `tty`/`vc_data` como coisas distintas.

> Isto realiza, já na fundação, a **Fase 6** do `vga-roadmap.md`. As Fases 2–5
> (cursor, scroll por hardware, cor dinâmica, ANSI) continuam valendo — são
> melhorias *dentro* do `VgaText`, e o `port.rs` já está pronto para elas.

## A camada `Framebuffer` (onde VGA e multi-arch se encontram)

A trait `Framebuffer` é o que torna o VGA **extensível** e, ao mesmo tempo, o que
dará saída gráfica em outras arquiteturas. Mesma abstração, dois objetivos:

```
                       trait Console
                       ▲           ▲
                 VgaText       FbConsole<F: Framebuffer>   ← desenha glifos em pixels
                 (texto)            │ genérico sobre F
                          ┌─────────┼──────────┐
                    Vga13h      Vga12h     DtFramebuffer
                   (linear)    (planar)   (ARM/RISC-V via Device Tree)
                   [FASE B]    [FASE B]   [jornada multi-arch]
```

- **Novo modo VGA / nova GPU** = implementar `Framebuffer`. O `FbConsole` desenha
  texto por cima de graça.
- **Novo backend de texto** (serial) = implementar `Console` direto.
- O `printk` nunca muda. É exatamente o par `vgacon`/`fbcon` do Linux.

## Como o Linux faz (referência cruzada)

| Peça | Linux | `izanagi` |
|---|---|---|
| Código por-arch | `arch/x86`, `arch/arm64`, `arch/riscv` com a mesma interface via headers `asm/` | `src/arch/<arch>/` com a mesma superfície (`init`, `halt`, ...) |
| Núcleo genérico | `kernel/`, `mm/`, `fs/`, `drivers/` não sabem a arch | `src/kernel/` |
| Selecionar arch | `Kconfig` + `ARCH=` no build | targets `*-unknown-none` + `#[cfg(target_arch)]` |
| Descobrir hardware | x86: legado/ACPI sabe `0xB8000`. ARM/RISC-V: **Device Tree** diz onde está cada device | (futuro) crate `fdt` parseia o DTB |
| Console de texto | `vgacon` e `fbcon` implementam a mesma `struct console` | `VgaText` e `FbConsole` implementam `Console` |
| Acesso a registrador portável | `readl/writel/ioremap` escondem MMIO vs PMIO | accessor por-arch (VGA, sendo x86-only, usa `port.rs` cru) |

A lição que se repete em todo subsistema do Linux (e vai se repetir aqui em
filesystems, block, net...): **uma interface estável no meio, implementações
intercambiáveis nas pontas.**

## Fluxo de boot atual (x86, `bootloader 0.9` / BIOS)

```
  bootloader 0.9  ── pula para ──►  _start            (arch/x86_64/boot.rs)
                                       │
                                       ├─ arch::init()           (nada ainda)
                                       └─ kernel_main()          (main.rs)
                                            ├─ register_console(&VGA)
                                            ├─ println!("Hello World!")
                                            └─ panic!  →  println! + arch::halt()
```

O símbolo de entrada é `_start` (convenção do linker). `#[unsafe(no_mangle)]`
mantém o nome intacto para o bootloader encontrá-lo. `#![no_main]` diz ao Rust
para não exigir um `main` padrão.

## Próximas jornadas (fora da Fase A)

São arcos de aprendizado próprios — a árvore já está *preparada* para eles, mas
não os iniciamos ainda.

- **Profundidade de VGA (Fase B)** — `Framebuffer` para o modo 13h (320×200, 256
  cores, linear @`0xA0000`) e depois o 12h (640×480, planar); `FbConsole` + fonte
  bitmap. Ver o `println!` virar glifos em modo gráfico sem o `printk` saber.
- **Multi-arquitetura (RISC-V primeiro)** — adicionar `src/arch/riscv64/`, target
  `riscv64gc-unknown-none-elf` (built-in), boot stub lendo o ponteiro do DTB,
  parse de Device Tree (`fdt`), e um console UART/SBI. O facade torna isso
  **aditivo**: nada do código genérico muda. ARM (`aarch64-unknown-none`) depois.
- **Bootloaders & UEFI** — uma jornada só de trade-offs: `bootloader 0.9` vs
  GRUB/Multiboot2 vs UEFI/GOP vs Limine. Decisão adiada de propósito.

## Glossário rápido

| Termo | Significado |
|---|---|
| **facade (arch)** | módulo que re-exporta a arch ativa via `#[cfg]`, dando uma superfície única ao código genérico (≈ headers `asm/` do Linux) |
| **inversão de dependência** | o subsistema (printk) depende de uma *trait*, não de drivers concretos; os drivers se registram nele |
| **policy vs mechanism** | "o quê/quando decidir" (printk) vs "como cuspir bytes no hardware" (driver) |
| **interior mutability** | mutar estado por trás de `&self` (via `Mutex`), em vez de exigir `&mut self` |
| **Device Tree (DTB)** | descrição em árvore do hardware que o firmware passa ao kernel em ARM/RISC-V (substitui os endereços "mágicos" do x86 legado) |
| **MMIO / PMIO** | Memory-Mapped I/O (`mov`/ponteiro) vs Port-Mapped I/O (`in`/`out`, só x86) |
