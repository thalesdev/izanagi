# Do `0xB8000` ao Subsistema de Console — Guia de Estudo do Driver VGA

> Material de estudo para evoluir o driver de texto deste kernel passo a passo.
> Não é um checklist de tarefas: é um guia para **entender** o que está
> acontecendo no hardware e nos kernels reais, e então implementar você mesmo.
> Cada fase tem: o conceito, o hardware concreto, diagramas, como Linux/Windows/
> BSD resolvem o mesmo problema, e links para fontes primárias.

> **Atualização (reestruturação da árvore).** O projeto foi reorganizado — veja
> [architecture.md](architecture.md). Duas consequências para este guia: (1) a
> **Fase 6 (inversão para `trait Console`) já está feita**, trazida para a
> fundação, e o `printk` vive em `src/kernel/printk.rs`; (2) os arquivos do VGA
> texto agora estão em `src/drivers/vga/text/`, e o port I/O já tem lar em
> `src/arch/x86_64/port.rs`. As fases abaixo foram anotadas com o que mudou.

## Índice

- [Como usar este guia](#como-usar-este-guia)
- [Parte 0 — Fundamentos](#parte-0--fundamentos)
  - [0.1 Um pouco de história: por que `0xB8000`?](#01-um-pouco-de-história-por-que-0xb8000)
  - [0.2 Como o modo texto funciona](#02-como-o-modo-texto-funciona)
  - [0.3 Memory-mapped I/O vs Port-mapped I/O](#03-memory-mapped-io-vs-port-mapped-io)
  - [0.4 Os grupos de registradores do VGA](#04-os-grupos-de-registradores-do-vga)
  - [0.5 O elefante na sala: isso é legado](#05-o-elefante-na-sala-isso-é-legado)
- [Mapa das fases](#mapa-das-fases)
- [Fase 1 — Output básico (já feito)](#fase-1--output-básico-já-feito)
- [Fase 2 — Cursor de hardware](#fase-2--cursor-de-hardware)
- [Fase 3 — Scroll por hardware](#fase-3--scroll-por-hardware)
- [Fase 4 — Cor e atributos dinâmicos](#fase-4--cor-e-atributos-dinâmicos)
- [Fase 5 — Sequências de escape ANSI](#fase-5--sequências-de-escape-ansi)
- [Fase 6 — Interface de console (a inversão) ✅ feito](#fase-6--interface-de-console-a-inversão)
- [Fase 7 — Console serial](#fase-7--console-serial)
- [Fase 8 — Terminais virtuais](#fase-8--terminais-virtuais)
- [Apêndice A — Biblioteca de links](#apêndice-a--biblioteca-de-links)
- [Apêndice B — Glossário](#apêndice-b--glossário)

---

## Como usar este guia

1. Leia a **Parte 0** inteira antes de codar qualquer coisa. Sem ela, as fases
   viram "copiar mágica de registrador". Com ela, cada porta faz sentido.
2. Em cada fase, antes de escrever código: leia a seção de **hardware**, desenhe
   o diagrama no papel, e só então implemente.
3. Teste cada fase no QEMU isoladamente. Um kernel é implacável: sem testes
   incrementais, você acumula bugs invisíveis.
4. Os blocos de código são **esqueletos conceituais**, não soluções prontas. A
   ideia é você preencher — é assim que se aprende.

**Pré-requisitos mentais:** entender ponteiros, bits/máscaras, e o básico de
assembly x86 (o que é uma instrução, um registrador). Não precisa ser expert.

---

## Parte 0 — Fundamentos

### 0.1 Um pouco de história: por que `0xB8000`?

O endereço `0xB8000` não é arbitrário — é herança arqueológica da IBM PC (1981).

```
Linha do tempo dos adaptadores de vídeo IBM:

  1981  MDA   (Monochrome Display Adapter)   texto mono   buffer @ 0xB0000
  1981  CGA   (Color Graphics Adapter)       texto+cor    buffer @ 0xB8000
  1984  EGA   (Enhanced Graphics Adapter)    cor          superset do CGA
  1987  VGA   (Video Graphics Array)          cor          superset do EGA
        └─> compatível para trás: ainda responde em 0xB8000 no modo texto
```

Quando a IBM mapeou a memória do PC original, reservou a faixa `0xA0000–0xBFFFF`
(o "UMA", *Upper Memory Area*) para vídeo. O MDA monocromático ficou em
`0xB0000`; o CGA colorido em `0xB8000`. Todo adaptador desde então mantém
compatibilidade para trás — por isso até uma GPU moderna, ao bootar em modo
texto legado, ainda expõe um buffer em `0xB8000`. Você está literalmente
programando uma interface de 1981.

```
Memória física baixa do PC (real mode, primeiro 1 MB):

0x00000 ┌─────────────────────────┐
        │ RAM convencional (640K) │
0xA0000 ├─────────────────────────┤ ← início da UMA
        │ VGA graphics framebuffer│
0xB0000 ├─────────────────────────┤
        │ MDA mono text           │
0xB8000 ├─────────────────────────┤ ← VOCÊ ESTÁ AQUI (CGA/VGA color text)
        │ ...                     │
0xC0000 ├─────────────────────────┤
        │ Video BIOS ROM          │
        │ ...                     │
0xFFFFF └─────────────────────────┘ ← fim do 1º MB
```

📖 Leia: [Wikipedia — VGA text mode](https://en.wikipedia.org/wiki/VGA_text_mode),
[OSDev — VGA Hardware](https://wiki.osdev.org/VGA_Hardware).

### 0.2 Como o modo texto funciona

No modo texto (modo 3 do BIOS: 80×25, 16 cores), você **não** desenha pixels.
Você escreve **células**, e o hardware do VGA tem um *character generator* que
converte cada código de caractere em pixels usando uma fonte armazenada na
própria placa.

Cada célula são **2 bytes**:

```
Uma célula de tela = 2 bytes (16 bits)

   byte 1 — atributo            byte 0 — caractere
 ┌───┬───────────┬─────────┐  ┌─────────────────────┐
 │ 7 │  6  5  4  │ 3 2 1 0 │  │  code point CP437   │
 │ B │    BG     │   FG    │  │     0x00 .. 0xFF    │
 └─┬─┴─────┬─────┴────┬────┘  └─────────────────────┘
   │       │          └── foreground: cor 0–15
   │       └───────────── background: cor 0–7
   └───────────────────── blink (ou bg brilhante, depende de um reg de modo)
```

O caractere **não é ASCII** — é **Code Page 437** (o conjunto OEM da IBM PC),
que tem os 128 ASCII + caracteres de box-drawing (`│ ─ ┌ ┐`), gregos, etc. É por
isso que no seu `write_string` os bytes fora de `0x20..=0x7e` viram `0xfe` (o
caractere `■`): você está filtrando para não imprimir lixo de CP437.

A tela inteira é um array linear na memória:

```
0xB8000 ┌──────────────────── 80 colunas ─────────────────────┐
        │ (0,0)(0,1)(0,2) .......................... (0,79)    │ linha 0
        │ (1,0) ...                                            │ linha 1
        │  ...                                                 │   ...
        │ (24,0) ................................... (24,79)   │ linha 24
        └──────────────────────────────────────────────────────┘

   offset_em_bytes(linha, coluna) = (linha * 80 + coluna) * 2

   Total: 80 * 25 * 2 = 4000 bytes (de um plano de 32 KB)
```

É exatamente o seu `struct Buffer { chars: [[ScreenChar; 80]; 25] }`. O `Volatile`
existe porque o compilador, vendo você escrever num endereço de RAM e nunca ler
de volta, "otimizaria" eliminando suas escritas — mas aquele endereço é hardware,
e a escrita *tem* efeito colateral visível. `volatile` proíbe essa otimização.

📖 Leia: [OSDev — Printing to Screen](https://wiki.osdev.org/Printing_To_Screen),
[Wikipedia — Code page 437](https://en.wikipedia.org/wiki/Code_page_437).

### 0.3 Memory-mapped I/O vs Port-mapped I/O

Esta é a distinção mais importante da Parte 0, porque a partir da Fase 2 você
sai do mundo confortável da memória e entra no mundo das **portas**.

O x86 tem **dois espaços de endereçamento separados**:

```
                          ┌──────────────────────────────────────┐
                          │                CPU                     │
                          └──────────────┬───────────┬───────────┘
                                         │           │
            espaço de MEMÓRIA  ──────────┘           └────── espaço de I/O
            (instruções mov)                          (instruções in/out)
                  │                                            │
       ┌──────────┼──────────┐                      ┌──────────┼──────────┐
       ▼          ▼          ▼                      ▼          ▼          ▼
      RAM    VGA buffer    ROM               CRTC VGA      UART      teclado
            @0xB8000                       @0x3D4/0x3D5   @0x3F8     @0x60/0x64
```

- **Memory-mapped I/O (MMIO):** o dispositivo aparece no espaço de endereços de
  memória. Você fala com ele usando `mov` (em Rust: escrita num ponteiro). O
  buffer VGA em `0xB8000` é MMIO. **Você já faz isso.**
- **Port-mapped I/O (PMIO):** um espaço *separado* de 64K "portas", acessível
  só pelas instruções `in` e `out`. O cursor, o scroll, a UART serial e o
  teclado vivem aqui.

Por que dois espaços? Decisão de design do 8086 (1978): economizava pinos de
endereço e dava ao hardware um jeito barato de distinguir "isso é RAM" de "isso
é um periférico". Arquiteturas RISC modernas (ARM, RISC-V) abandonaram isso e
usam **só MMIO** — o x86 mantém PMIO por compatibilidade.

Em Rust, `in`/`out` exigem `inline asm` ou um wrapper. O crate `x86_64` já dá:

```rust
use x86_64::instructions::port::Port;

let mut port: Port<u8> = Port::new(0x3D4);
unsafe { port.write(0x0F_u8); }     // OUT — seleciona registrador
let value: u8 = unsafe { port.read() }; // IN  — lê de volta
```

📖 Leia: [OSDev — I/O Ports](https://wiki.osdev.org/I/O_Ports),
[felixcloutier — instrução `out`](https://www.felixcloutier.com/x86/out),
[crate `x86_64::Port`](https://docs.rs/x86_64/latest/x86_64/instructions/port/struct.Port.html).

### 0.4 Os grupos de registradores do VGA

O VGA tem **muito mais** que o buffer de memória. Internamente são ~5 grupos de
registradores, cada um com seu par de portas índice/dado:

| Grupo | Função | Porta índice | Porta dado |
|---|---|---|---|
| **CRTC** (CRT Controller) | cursor, scroll, timing, resolução | `0x3D4` | `0x3D5` |
| Sequencer | clocking, planos de memória | `0x3C4` | `0x3C5` |
| Graphics Controller | modo gráfico, mapeamento de planos | `0x3CE` | `0x3CF` |
| Attribute Controller | paleta, modo de blink | `0x3C0` | `0x3C0`/`0x3C1` |
| DAC (Color) | converte índice de cor → RGB analógico | `0x3C8` | `0x3C9` |

No modo texto, **quase tudo o que você vai mexer é o CRTC** (`0x3D4`/`0x3D5`).
O padrão de uso é sempre o mesmo — **índice + dado**:

```
Para escrever o valor V no registrador interno N do CRTC:

   out 0x3D4, N     ; "quero falar com o registrador N"  (porta de índice)
   out 0x3D5, V     ; "o valor dele é V"                  (porta de dado)

Para ler o registrador N:

   out 0x3D4, N     ; seleciona
   in  al, 0x3D5    ; lê o valor atual
```

Registradores do CRTC que importam para nós:

| Reg | Nome | Usado na fase |
|---|---|---|
| `0x0A` | Cursor Start (scanline inicial + bit de enable) | 2 |
| `0x0B` | Cursor End (scanline final) | 2 |
| `0x0C` | Start Address High | 3 |
| `0x0D` | Start Address Low | 3 |
| `0x0E` | Cursor Location High | 2 |
| `0x0F` | Cursor Location Low | 2 |

> **Nota sobre mono vs cor:** em placas monocromáticas o CRTC fica em `0x3B4`/
> `0x3B5`. O bit 0 do registrador "Miscellaneous Output" (`0x3CC`) diz qual está
> ativo. Em modo cor (o nosso caso) é sempre `0x3D4`/`0x3D5`.

📖 A referência canônica e gratuita de todos esses registradores é o **FreeVGA**:
[FreeVGA — CRTC Registers](http://www.osdever.net/FreeVGA/vga/crtcreg.htm),
[FreeVGA — home](http://www.osdever.net/FreeVGA/home.htm).

### 0.5 O elefante na sala: isso é legado

Honestidade técnica: **o modo texto VGA está morrendo.** Máquinas UEFI modernas
bootam direto em modo gráfico via **GOP** (Graphics Output Protocol) — não existe
buffer em `0xB8000`, existe um framebuffer linear de pixels RGB. O Linux usa
`fbcon`/DRM; o bootloader `bootloader 0.9` deste projeto te dá VGA texto porque
faz boot BIOS/legado.

Então por que estudar isso? Porque **os conceitos transferem 100%**:

- Cursor, scroll, cores, ANSI, console abstrato, serial, VTs — tudo isso existe
  igual num framebuffer gráfico, só que você desenha glifos em pixels em vez de
  escrever células.
- A **arquitetura** (Fases 6–8) é idêntica independente do hardware de saída. É
  exatamente por isso que o `printk` do Linux não liga se o backend é VGA texto,
  framebuffer, ou serial.

Trate o VGA texto como o "hello world" do output de kernel: simples o bastante
para você focar nos conceitos, não nos pixels.

📖 Quando quiser dar o salto: [OSDev — Drawing In a Linear Framebuffer](https://wiki.osdev.org/Drawing_In_a_Linear_Framebuffer),
[GOP](https://wiki.osdev.org/GOP).

---

## Mapa das fases

```
 Parte 0  Fundamentos (ler antes de tudo)
    │
    ▼
 Fase 1  output básico ............................. [✅ feito]
    │
    ▼
 Fase 2  cursor de hardware ........................ introduz PORT I/O ★
    │                                                (módulo reusado por todas)
    ├─────────────┐
    ▼             ▼
 Fase 3        Fase 4  cor dinâmica
 scroll HW        │
 [opcional]       ▼
               Fase 5  parser ANSI ................. máquina de estados
                  │
                  ▼
               Fase 6  trait Console ............... ✅ FEITO (na fundação)
                  │                                   (inversão de dependência)
                  ├─────────────┐
                  ▼             ▼
               Fase 7        Fase 8
               serial         terminais virtuais
            (2º backend)      (Ctrl+Alt+Fn)
```

Dois marcos: **Fase 2** te tira do conforto da memória e te dá port I/O (que
quase tudo depois usa). **Fase 6** te tira do "tenho um driver" e te leva a
"tenho uma arquitetura de subsistema" — e esse salto **já foi dado** na
reestruturação da árvore (ver [architecture.md](architecture.md)).

---

## Fase 1 — Output básico (já feito)

**Status:** ✅ implementado em `src/drivers/vga/text/` (movido na reestruturação).

O que você já domina, e que é a fundação de tudo:

- `write_byte` / `write_string` — escreve células em `0xB8000` (MMIO).
- `new_line` + scroll por software — copia linhas 1..25 → 0..24, limpa a última.
- `println!`/`print!` via `core::fmt::Write` — formatação genérica (hoje
  despachados pelo `printk` em `src/kernel/printk.rs`; ver Fase 6).
- Estado global seguro com `lazy_static` + `spin::Mutex`.

**Por que o `Mutex` é `spin` e não o da `std`?** Porque não temos `std`, não
temos scheduler ainda, e não dá para "dormir" uma thread bloqueada. Um spinlock
fica girando num loop até conseguir o lock. É primitivo e até perigoso (deadlock
se você tomar o lock dentro de uma interrupção que rodou enquanto o lock estava
tomado) — um problema real que o blog_os discute e que você vai reencontrar na
Fase 7. Guarde isso.

📖 A série que inspira essa fase: [blog_os — VGA Text Mode](https://os.phil-opp.com/vga-text-mode/).

**Limitações que motivam as próximas fases:**
1. O cursor de hardware não acompanha a escrita. → Fase 2
2. O scroll copia 2000 células por newline. → Fase 3
3. A cor é fixa, definida uma vez. → Fase 4

---

## Fase 2 — Cursor de hardware

### Conceito

O VGA mantém um **cursor piscante de hardware** — aquele bloco/underline que pisca.
Ele é independente do que você escreve: vive em dois registradores do CRTC que
guardam a *posição linear* do cursor. Hoje seu kernel escreve texto mas o cursor
fica parado, porque você nunca atualiza esses registradores.

Esta fase é o seu **primeiro contato com port I/O**. O módulo de portas já existe
(`src/arch/x86_64/port.rs`, criado na fundação) e será reusado nas Fases 3 e 7.

### Hardware

```
Posição do cursor é um offset linear de 16 bits:

   pos = linha * 80 + coluna          (0 .. 1999)

Esse valor de 16 bits é dividido em dois registradores de 8 bits do CRTC:

   reg 0x0F  =  pos & 0xFF          (byte baixo)
   reg 0x0E  = (pos >> 8) & 0xFF    (byte alto)

Forma do cursor + enable (registradores 0x0A / 0x0B):

   0x0A  Cursor Start:  bits 0-4 = scanline inicial
                        bit  5   = 1 DESLIGA o cursor
   0x0B  Cursor End:    bits 0-4 = scanline final
```

### O que implementar

1. **Módulo de portas reusável.** ✅ **Já existe** em `src/arch/x86_64/port.rs`
   (`outb`/`inb`), criado na fundação — hoje marcado `#[allow(dead_code)]` porque
   ninguém o usa ainda. É só passar a chamá-lo (ou usar o crate `x86_64` direto).

2. **Mover o cursor:**
   ```rust
   fn update_cursor(&self) {
       let pos: u16 = (self.row * BUFFER_WIDTH + self.column_position) as u16;
       let mut index = Port::<u8>::new(0x3D4);
       let mut data  = Port::<u8>::new(0x3D5);
       unsafe {
           index.write(0x0F); data.write((pos & 0xFF) as u8);
           index.write(0x0E); data.write((pos >> 8) as u8);
       }
   }
   ```
   Chame ao fim de cada `write_byte` e `new_line`. (Repare que isso pressupõe
   você rastrear a linha atual, não só a coluna — pequeno refactor do `Writer`.)

3. **Enable/disable e forma** (opcional, mas educativo): ler-modificar-escrever
   nos regs `0x0A`/`0x0B` preservando os bits que você não quer mexer. Isso
   ensina o padrão *read-modify-write* de registradores de hardware.

### Como os grandes fazem

- **Linux:** a manipulação de cursor de texto VGA está em
  [`drivers/video/console/vgacon.c`](https://elixir.bootlin.com/linux/latest/source/drivers/video/console/vgacon.c)
  — procure `vgacon_cursor()` e `write_vga()`. Você vai ver exatamente esses
  registradores `0x0E`/`0x0F` e o padrão índice/dado.
- **OSDev** tem um tutorial direto desta fase: [Text Mode Cursor](https://wiki.osdev.org/Text_Mode_Cursor).

### Conceitos que esta fase consolida

Port-mapped I/O na prática; o padrão índice+dado do CRTC; read-modify-write em
hardware; separar código arquitetura-específico (`arch/x86_64/port.rs`) do
genérico.

### Como testar

Escreva algum texto e confirme que o cursor piscante para logo **depois** do
último caractere, não no canto superior esquerdo.

---

## Fase 3 — Scroll por hardware

### Conceito

Seu scroll atual copia ~2000 células de memória a cada quebra de linha. O VGA tem
um jeito muito mais esperto: o registrador **Start Address** diz a partir de qual
offset da memória de vídeo o CRTC começa a desenhar. A memória de texto tem 32 KB
(cabem ~200 linhas de 80 colunas), bem mais que uma tela. Então você trata tudo
como um **buffer circular** e, para rolar, só **incrementa o start address** —
zero cópia.

```
Memória de texto (32 KB) >> uma tela (4000 bytes):

         ┌──────────────┐  ← Start Address = 0
         │ tela visível │
         │  (25 linhas) │
         ├──────────────┤  ← ao rolar 1 linha, Start Address += 80 células
         │              │
         │ espaço extra │     o CRTC simplesmente "desliza para baixo"
         │              │     sem ninguém copiar nada
         └──────────────┘
         quando chega no fim físico → wrap-around (a parte tricky)
```

### Hardware

```
   reg 0x0C  =  start_addr_em_células >> 8     (byte alto)
   reg 0x0D  =  start_addr_em_células & 0xFF   (byte baixo)

   (note: é em CÉLULAS, não em bytes)
```

### O que implementar

1. Manter um `start_offset` lógico e escrevê-lo nos regs `0x0C`/`0x0D`.
2. Em vez de copiar no `new_line`, incrementar `start_offset` por `BUFFER_WIDTH`.
3. **Wrap-around:** quando `start_offset + tela` ultrapassa a memória física,
   você precisa copiar uma vez (ou usar aritmética modular e escrever sempre na
   posição certa). Este é o caso de borda chato.
4. Lembrar de somar `start_offset` quando calcular a posição do **cursor** (Fase
   2) e quando escrever caracteres — tudo agora é relativo ao start.

### Como os grandes fazem

- **Linux:** `vgacon` tem scroll por hardware há décadas — veja `vgacon_scroll()`
  e `vgacon_set_origin()` em
  [`vgacon.c`](https://elixir.bootlin.com/linux/latest/source/drivers/video/console/vgacon.c).
  É literalmente o mesmo registrador de start address.

### ⚠️ Vale pular?

Sim, tranquilamente. O wrap-around tem casos de borda traiçoeiros, e o scroll por
software da Fase 1 funciona perfeitamente para um kernel didático. Trate esta
fase como um desafio opcional de otimização. **Recomendo fazer as Fases 4–6
antes** e voltar aqui se quiser.

### Conceitos que esta fase consolida

O VGA enxerga a própria memória como buffer circular; a diferença real entre
scroll por software (custo O(tela)) e por hardware (custo O(1)); aritmética
modular sobre buffer físico.

---

## Fase 4 — Cor e atributos dinâmicos

### Conceito

Hoje a cor é cravada na inicialização do `WRITER`
(`ColorCode::new(Color::Yellow, Color::Black)`). Para fazer logs coloridos (ex:
erros em vermelho) e, principalmente, para suportar ANSI na Fase 5, você precisa
mudar a cor **em runtime**.

Esta é a fase mais simples — não tem hardware novo, só design de API. Use-a para
respirar entre a Fase 3 (difícil) e a 5 (máquina de estados).

### O que implementar

1. `Writer::set_color(fg: Color, bg: Color)` que atualiza `self.color_code`.
2. Um helper que salva/restaura cor, útil para logs pontuais:
   ```rust
   pub fn with_color(&mut self, fg: Color, bg: Color, f: impl FnOnce(&mut Self)) {
       let saved = self.color_code;
       self.set_color(fg, bg);
       f(self);
       self.color_code = saved;
   }
   ```
3. (Opcional) Entender o **bit de blink vs bright-background.** O bit 7 do byte
   de atributo é ambíguo: por padrão faz o texto **piscar**; mas há um modo
   (controlado pelo Attribute Controller, reg `0x10`) em que ele vira o bit alto
   da cor de background, te dando 16 cores de fundo em vez de 8. Documentar essa
   escolha já é meio caminho para entender por que "fundo branco brilhante" às
   vezes não existe em modo texto.

### Como os grandes fazem

- O conceito de **níveis de log** do Linux (`KERN_ERR`, `KERN_WARNING`, …) é
  parente disso: o nível depois é mapeado para cor pelo console. Veja os macros
  em [`include/linux/kern_levels.h`](https://elixir.bootlin.com/linux/latest/source/include/linux/kern_levels.h).
  Guarde a ideia de "nível de severidade → apresentação" para a Fase 6.

### Conceitos que esta fase consolida

Encapsular estado mutável do driver; o padrão save/restore; a ambiguidade
histórica do bit de blink.

---

## Fase 5 — Sequências de escape ANSI

### Conceito

Terminais de verdade não recebem "comandos" — recebem um **fluxo de bytes** onde
certos bytes especiais (começando com `ESC`, `0x1B`) significam *controle* em vez
de *texto*. `\x1b[31m` não imprime "31m" em vermelho: é o comando "ativar
foreground vermelho". Interpretar isso transforma seu `write_string` numa
**máquina de estados finitos** — e te ensina como o seu próprio terminal,
agora mesmo, está funcionando.

Esse protocolo é o **ECMA-48 / ANSI X3.64**, popularizado pelos terminais DEC
**VT100** (1978). Por isso se fala "terminal VT100-compatível".

```
Anatomia de uma sequência CSI (Control Sequence Introducer):

   ESC  [   3 1   m
   │    │   │     │
   │    │   │     └── byte final: comando (m = SGR "set graphic rendition")
   │    │   └──────── parâmetros: números separados por ';'  (ex: "1;31")
   │    └──────────── '[' : abre uma sequência CSI
   └───────────────── ESC (0x1B): "o que vem agora é controle"
```

### Máquina de estados

```
                 byte normal
              ┌──────────────┐
              ▼              │
        ┌──────────┐  0x1B   ┌──────────┐  '['   ┌──────────┐
   ───> │  NORMAL  │────────>│  ESCAPE  │───────>│   CSI    │
        │          │         │          │        │ (junta   │
        │ imprime  │<────────│          │        │  params) │
        └──────────┘  outro  └──────────┘        └────┬─────┘
              ▲       (aborta)                         │ byte final
              │                                        │ (executa comando)
              └────────────────────────────────────────┘
```

### Sequências mínimas para começar

| Sequência | Efeito |
|---|---|
| `\x1b[0m` | reset de todos os atributos |
| `\x1b[30..37m` | cor de foreground (0=preto … 7=branco) |
| `\x1b[40..47m` | cor de background |
| `\x1b[1m` | "bold"/bright (mapeie para a cor brilhante) |
| `\x1b[2J` | limpar a tela inteira |
| `\x1b[H` | cursor para o canto superior esquerdo (0,0) |
| `\x1b[<l>;<c>H` | mover cursor para linha `l`, coluna `c` |

### O que implementar

1. Um enum `enum ParseState { Normal, Escape, Csi }` no `Writer` e um buffer
   pequeno para acumular os dígitos dos parâmetros.
2. No `write_byte`, ramificar pelo estado: em `Normal`, `0x1B` → vai pra
   `Escape`; em `Csi`, dígitos e `;` se acumulam, e um byte final executa.
3. Uma função que mapeia código ANSI → seu `enum Color` (cuidado: a ordem das
   cores ANSI **não** é a mesma do VGA; ANSI 1 = vermelho, VGA 4 = vermelho —
   você precisa de uma tabela de tradução).
4. Implementar primeiro só `m` (cor) e `2J`/`H` (clear/home). O resto vem depois.

### Como os grandes fazem

- **Linux:** o parser ANSI/VT do console vive em
  [`drivers/tty/vt/vt.c`](https://elixir.bootlin.com/linux/latest/source/drivers/tty/vt/vt.c)
  — procure `do_con_trol()` e `csi_J()`, `csi_m()`. É uma máquina de estados bem
  mais completa que a sua, mas a espinha é idêntica.
- A referência **exaustiva** de sequências de controle é o doc do xterm:
  [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html).
- O padrão formal: [ECMA-48](https://ecma-international.org/publications-and-standards/standards/ecma-48/).
  Resumo prático: [Wikipedia — ANSI escape code](https://en.wikipedia.org/wiki/ANSI_escape_code).

### Conceitos que esta fase consolida

Máquinas de estado finitas; parsing incremental byte-a-byte (você não tem a
string inteira, processa um byte por vez — igual a um parser de protocolo de
rede); a diferença entre dados e comandos in-band; a tabela de cores ANSI↔VGA.

---

## Fase 6 — Interface de console (a inversão)

**Status:** ✅ **feito na fundação** (reestruturação da árvore). A inversão já está
em [`src/kernel/printk.rs`](../src/kernel/printk.rs); as seções abaixo agora
documentam **como ficou** em vez de propor. Leia o conceito mesmo assim — é o
coração do guia. Ver também [architecture.md](architecture.md).

### Conceito — leia com atenção, é o coração do guia

Até aqui você tem um **driver**. A partir daqui você tem uma **arquitetura**.

Hoje o caminho é acoplado: `println!` → `_print()` → `WRITER` (VGA). O `_print`
**conhece** o VGA. Se amanhã você quiser logar na serial também, teria que mexer
no `_print`. Isso não escala.

Nos kernels reais, o subsistema de log **não conhece driver nenhum**. Ele expõe
uma **interface** (uma trait), e os drivers se *registram* como implementações.
O log itera sobre os registrados. É a **inversão de dependência**:

```
   ANTES (Fases 1–5)                  DEPOIS (Fase 6) — estilo Linux

   println!                            println!
      │                                   │
      ▼                                   ▼
   _print() ──────> VGA              printk() ──> lista de consoles registrados
      (hardcoded)                        │            │
                                         │            ├──> [dyn Console] VGA
   "o print conhece                      │            ├──> [dyn Console] Serial
    o driver"                            │            └──> [dyn Console] ...
                                         │
                              "o driver se registra no print;
                               o print não conhece nenhum driver"
```

Isso separa **policy** (o subsistema de log: formatar, gerenciar, decidir o quê)
de **mechanism** (o driver: como de fato cuspir bytes no hardware) — um dos
princípios mais centrais de design de kernel.

### Como os grandes fazem (esta seção é o ponto)

- **Linux** — o `printk` (`kernel/printk/printk.c`) escreve numa estrutura
  central e depois chama todos os consoles registrados. A interface é a
  `struct console`:
  - [`include/linux/console.h`](https://elixir.bootlin.com/linux/latest/source/include/linux/console.h)
    — veja `struct console` com seu ponteiro `void (*write)(struct console *, const char *, unsigned)`.
    É **exatamente** uma trait em C: ponteiros de função.
  - [`kernel/printk/printk.c`](https://elixir.bootlin.com/linux/latest/source/kernel/printk/printk.c)
    — veja `register_console()` e `console_unlock()` iterando os consoles.
  - Backends concretos: `vgacon`, `fbcon` (framebuffer), `serial8250`,
    `netconsole` (manda log pela rede!). Todos implementam a mesma `struct console`.

- **Windows NT** — a separação é ainda mais radical e vale conhecer:
  - O **HAL** (Hardware Abstraction Layer) isola o kernel do hardware da placa-mãe.
  - No boot, quem desenha na tela é o **`bootvid.dll`** (Boot Video Driver) — um
    driver minúsculo que faz o equivalente ao seu VGA. Implementação legível na
    reimplementação open-source ReactOS:
    [reactos/drivers/base/bootvid](https://github.com/reactos/reactos/tree/master/drivers/base/bootvid).
  - O log de debug do kernel (`DbgPrint`/`KdPrint`) **não vai para a tela** — vai
    para o **kernel debugger (KD)** por serial, USB ou rede (KDNET), e você lê no
    WinDbg de outra máquina. Filosofia diferente do Linux (que mostra na tela):
    [Microsoft — Kernel debugging](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/).

- **FreeBSD** — o console é abstraído pela interface `cnops`/`consdev`; drivers
  como `vt(4)` (o console moderno) e o histórico `syscons` se plugam nela. Mesma
  ideia, outro nome.

A lição: **três kernels, três nomes, uma única ideia** — uma interface estável no
meio e drivers intercambiáveis nas pontas.

### Como ficou implementado

Tudo em [`src/kernel/printk.rs`](../src/kernel/printk.rs):

1. Os macros `print!`/`println!` e o despacho saíram do VGA e vieram para cá; o
   antigo `_print` agora **itera sobre os consoles registrados**.
2. A trait — repare nas diferenças para o esboço original (`&self` no lugar de
   `&mut self`, `Sync` no lugar de `Send`):
   ```rust
   pub trait Console: Sync {
       fn write_str(&self, s: &str);
       fn clear(&self);
   }
   ```
   **Por que `&self`/`Sync`?** Para guardar `&'static dyn Console` compartilhado
   sem `&'static mut` (um pesadelo de borrow-checker). Cada backend guarda o
   estado mutável atrás da própria trava (*interior mutability*): o `VgaText` é um
   tipo *zero-sized* que delega para um `Mutex<Writer>` global.
3. O VGA implementa via o wrapper `VgaText` (em `src/drivers/vga/text/writer.rs`).
4. Os consoles ficam numa **tabela de tamanho fixo** (`[Option<&dyn Console>; 4]`),
   não um `Vec` — ainda não temos alocador. `_print` itera os ocupados.
5. `register_console(&VGA)` é chamado no `kernel_main` (em `src/main.rs`).

Detalhe que vale ouro: como `Console::write_str` é `&self` mas `core::fmt` precisa
de `fmt::Write` (`&mut self`), um adapter `ConsoleWriter` faz a ponte — assim
reaproveitamos toda a formatação sem alocar nada.

> **`Console` é output-only**, igual ao `struct console` do Linux. O console
> *interativo* (input/stdin) é outra camada, maior — o TTY/VT — que é a **Fase 8**
> deste guia. "Console" aqui = sink de saída do printk.

### Estrutura de diretórios resultante (a real, hoje)

```
src/
├── main.rs              ← kernel_main(): registra os consoles
├── arch/x86_64/
│   ├── boot.rs          ← _start
│   └── port.rs          ← port I/O (lar da Fase 2)
├── kernel/
│   └── printk.rs        ← trait Console + register_console + _print (policy)
└── drivers/
    ├── vga/text/        ← VgaText: um backend de Console (mechanism)
    ├── video/           ← trait Framebuffer (p/ Fase B; o "fbcon" vem aqui)
    └── serial/          ← outro backend de Console (Fase 7, a criar)
```

### Conceitos que esta fase consolida

Trait objects (`dyn Trait`) e despacho dinâmico; inversão de dependência;
separação policy/mechanism; o conceito de "registrar um driver numa interface"
— que você vai reencontrar em *todo* subsistema de kernel (filesystems, drivers
de bloco, protocolos de rede…).

---

## Fase 7 — Console serial

### Conceito

A porta serial (UART) é o **canal de debug clássico** de kernels. Por quê?

- Funciona **antes** de quase tudo: não precisa de GPU, driver gráfico, nem nada.
- Com o QEMU, a saída serial cai **direto no terminal do host** (`-serial stdio`),
  então você lê logs como texto comum — e captura em arquivo para CI/testes.
- É como você debuga um kernel real que travou antes de a tela inicializar.

Esta fase também **valida a Fase 6**: se sua abstração de console estiver boa,
adicionar a serial é só implementar a trait `Console` de novo, **sem tocar no
`printk`**. Se você precisar mexer no `printk`, a abstração da Fase 6 vazou.

### Hardware: UART 16550, base `0x3F8` (COM1)

A UART é toda port-mapped. Os registradores são offsets a partir da base:

| Porta | Registrador | Uso |
|---|---|---|
| `0x3F8` | Data / Divisor Latch Low | byte de dados (ou divisor, se DLAB=1) |
| `0x3F9` | Int Enable / Divisor Latch High | interrupções (ou divisor alto) |
| `0x3FA` | FIFO Control | liga/limpa os FIFOs |
| `0x3FB` | Line Control (LCR) | bit DLAB, tamanho de palavra, paridade |
| `0x3FC` | Modem Control | |
| `0x3FD` | Line Status (LSR) | bit 5 = "pronto para transmitir" |

```
Transmitir um byte (polling):

   1. espere até LSR (0x3FD) bit 5 == 1   ; "transmit holding register empty"
   2. escreva o byte em 0x3F8

Configurar 8N1 a 38400 baud (exemplo):

   out 0x3FB, 0x80    ; DLAB=1 (próximas escritas em 0x3F8/9 são o divisor)
   out 0x3F8, divisor_low
   out 0x3F9, divisor_high
   out 0x3FB, 0x03    ; DLAB=0, 8 bits, sem paridade, 1 stop bit (8N1)
   out 0x3FA, 0xC7    ; liga e limpa FIFO
```

"8N1" = 8 bits de dados, No parity, 1 stop bit — a config serial mais comum.

### O que implementar

1. `src/drivers/serial/mod.rs` com `SerialPort::init()` e `SerialPort::send(u8)`
   (use os ports da Fase 2). Ou use o crate pronto
   [`uart_16550`](https://docs.rs/uart_16550/) para comparar com a sua.
2. Implementar a trait `Console` (`&self`, como ficou na fundação) para a serial —
   provavelmente um `SerialPort` atrás de um `Mutex`, espelhando o `VgaText`.
3. `register_console()` da serial no boot.
4. Configurar o QEMU com `-serial stdio` e ver `println!` aparecer **na tela e no
   terminal do host ao mesmo tempo**, sem o `printk` saber a diferença. Esse é o
   momento "aha" da arquitetura.

> ⚠️ Cuidado com o spinlock (lembra da Fase 1?): se uma interrupção tentar logar
> enquanto o lock do console está tomado, você trava. Kernels reais têm caminhos
> de log "lockless" ou que reentram com cuidado. Por ora, evite logar dentro de
> handlers de interrupção.

### Como os grandes fazem

- **Linux:** o driver serial genérico é o `8250`
  ([`drivers/tty/serial/8250/`](https://elixir.bootlin.com/linux/latest/source/drivers/tty/serial/8250/)).
  E existe o **`earlycon`/`earlyprintk`**: um console serial mínimo que sobe
  *antes* do resto, exatamente pelo motivo desta fase. Doc:
  [Serial console](https://www.kernel.org/doc/html/latest/admin-guide/serial-console.html).
- **Windows:** o KD (kernel debugger) sobre serial é a forma canônica de debugar
  o boot do NT — mesma motivação.
- **blog_os** tem um capítulo exatamente disso, incluindo rodar testes do kernel
  via serial + sair do QEMU programaticamente:
  [Testing](https://os.phil-opp.com/testing/).

### Conceitos que esta fase consolida

UART e polling de status; "early console"; a prova viva de que a abstração da
Fase 6 funciona (um `printk`, dois hardwares); a armadilha do lock em contexto de
interrupção.

---

## Fase 8 — Terminais virtuais

### Conceito

Múltiplas "telas" independentes, cada uma com seu próprio conteúdo, cursor e cor,
multiplexando **um único** hardware de vídeo. Trocar de terminal virtual (VT) é
salvar o estado do atual e restaurar o do alvo. É o que o `Ctrl+Alt+F1..F6` faz
no Linux: vários consoles de texto sobre uma placa só.

> **A ligação com a Fase 6:** esta é a camada que o `Console` *output-only* não
> cobre de propósito — estado de tela por terminal e (depois, com teclado) input.
> É a porta de entrada do TTY/VT, o "console interativo" de verdade, distinto do
> sink de saída do printk.

```
    N buffers em RAM                       hardware (uma tela)
   ┌─────────────┐ VT0                     ┌──────────────┐
   │ conteúdo +  │                         │              │
   │ cursor + cor│──┐                      │   0xB8000    │
   └─────────────┘  │  switch_to(1)        │   (o VT      │
   ┌─────────────┐  │  ┌──────────────┐    │   ativo)     │
   │     VT1     │──┼─>│ salva o atual│───>│              │
   └─────────────┘  │  │ restaura VT1 │    └──────────────┘
   ┌─────────────┐  │  └──────────────┘
   │     VT2     │──┘
   └─────────────┘
```

### O que implementar

1. Um array de `N` estruturas "tela virtual", cada uma com um backing buffer em
   RAM (`[[ScreenChar; 80]; 25]`), posição de cursor e cor.
2. Escritas vão para o buffer em RAM do VT **ativo**; só o ativo é espelhado em
   `0xB8000` (ou, se combinou com a Fase 3, você aponta o start address).
3. `switch_to(vt: usize)`: copia o estado atual para o buffer do VT atual e
   restaura o do alvo (incluindo a posição do cursor de hardware da Fase 2).
4. Mais tarde, quando tiver **teclado + interrupções**, disparar a troca por
   combinação de teclas — aí você terá o `Ctrl+Alt+Fn` de verdade.

### Como os grandes fazem

- **Linux:** o subsistema de VT é o
  [`drivers/tty/vt/vt.c`](https://elixir.bootlin.com/linux/latest/source/drivers/tty/vt/vt.c),
  parte da **camada TTY**. Cada VT é uma struct `vc_data` (virtual console). A
  função `redraw_screen()` / `set_console()` faz a troca. É a generalização
  exata do que você vai construir.
- Isso te coloca na porta de entrada da **camada TTY**, um dos subsistemas mais
  notoriamente complexos do Unix. Leitura clássica (e divertida) sobre o porquê
  dessa complexidade: [The TTY demystified](https://www.linusakesson.net/programming/tty/).

### Conceitos que esta fase consolida

Multiplexação de um recurso de hardware entre vários "clientes" lógicos;
save/restore de estado; a fundação conceitual de TTYs e sessões.

---

## Apêndice A — Biblioteca de links

### Fundamentos e referência de hardware VGA
- [OSDev — VGA Hardware](https://wiki.osdev.org/VGA_Hardware) — visão geral dos grupos de registradores.
- [FreeVGA](http://www.osdever.net/FreeVGA/home.htm) — **a** referência clássica e completa. [CRTC Registers](http://www.osdever.net/FreeVGA/vga/crtcreg.htm).
- [OSDev — Printing to Screen](https://wiki.osdev.org/Printing_To_Screen).
- [OSDev — Text Mode Cursor](https://wiki.osdev.org/Text_Mode_Cursor).
- [OSDev — I/O Ports](https://wiki.osdev.org/I/O_Ports) e [Inline Assembly](https://wiki.osdev.org/Inline_Assembly).
- [Wikipedia — VGA text mode](https://en.wikipedia.org/wiki/VGA_text_mode) · [Code page 437](https://en.wikipedia.org/wiki/Code_page_437).

### Serial / UART
- [OSDev — Serial Ports](https://wiki.osdev.org/Serial_Ports).
- [Wikipedia — 16550 UART](https://en.wikipedia.org/wiki/16550_UART).
- [crate `uart_16550`](https://docs.rs/uart_16550/) · [Linux serial console doc](https://www.kernel.org/doc/html/latest/admin-guide/serial-console.html).

### ANSI / Terminais
- [Wikipedia — ANSI escape code](https://en.wikipedia.org/wiki/ANSI_escape_code) (ótimo resumo prático).
- [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html) (a referência exaustiva).
- [ECMA-48](https://ecma-international.org/publications-and-standards/standards/ecma-48/) (o padrão formal).
- [VT100 User Guide](https://vt100.net/docs/vt100-ug/) (a origem histórica).
- [The TTY demystified](https://www.linusakesson.net/programming/tty/).

### Código-fonte de kernels reais (use o elixir.bootlin para navegar o Linux)
- Linux `printk`: [`kernel/printk/printk.c`](https://elixir.bootlin.com/linux/latest/source/kernel/printk/printk.c).
- Linux interface de console: [`include/linux/console.h`](https://elixir.bootlin.com/linux/latest/source/include/linux/console.h).
- Linux VGA console: [`drivers/video/console/vgacon.c`](https://elixir.bootlin.com/linux/latest/source/drivers/video/console/vgacon.c).
- Linux VT/console: [`drivers/tty/vt/vt.c`](https://elixir.bootlin.com/linux/latest/source/drivers/tty/vt/vt.c).
- Linux serial 8250: [`drivers/tty/serial/8250/`](https://elixir.bootlin.com/linux/latest/source/drivers/tty/serial/8250/).
- ReactOS (NT-like, legível) bootvid: [drivers/base/bootvid](https://github.com/reactos/reactos/tree/master/drivers/base/bootvid).
- Windows kernel debugging: [Microsoft Docs](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/).

### Rust no bare metal (alinhado com a stack deste projeto)
- [blog_os — VGA Text Mode](https://os.phil-opp.com/vga-text-mode/) (Fase 1).
- [blog_os — Testing](https://os.phil-opp.com/testing/) (serial + QEMU, Fase 7).
- [crate `x86_64`](https://docs.rs/x86_64/) (port I/O, estruturas de CPU).
- [The Embedonomicon](https://docs.rust-embedded.org/embedonomicon/) (Rust sem `std`, do zero).

---

## Apêndice B — Glossário

| Termo | Significado |
|---|---|
| **MMIO** | Memory-Mapped I/O — falar com hardware via endereços de memória (`mov`). |
| **PMIO / Port I/O** | Port-Mapped I/O — espaço separado acessado por `in`/`out`. |
| **CRTC** | CRT Controller — o grupo de registradores VGA do cursor/scroll/timing. |
| **CP437** | Code Page 437 — o conjunto de caracteres OEM da IBM PC (não é ASCII puro). |
| **CSI** | Control Sequence Introducer — o `ESC [` que abre comandos ANSI. |
| **SGR** | Select Graphic Rendition — o comando ANSI `m` (cores/estilo). |
| **UART** | Universal Async Receiver/Transmitter — o chip da porta serial (16550). |
| **8N1** | 8 data bits, No parity, 1 stop bit — config serial padrão. |
| **DLAB** | Divisor Latch Access Bit — bit do LCR que troca o significado dos regs da UART. |
| **VT** | Virtual Terminal — uma das múltiplas telas de texto sobre um hardware. |
| **TTY** | Teletypewriter — a camada Unix de terminais; herdou o nome dos teletipos. |
| **policy vs mechanism** | "o quê/quando decidir" (policy) vs "como executar" (mechanism). |
| **GOP** | Graphics Output Protocol — o framebuffer do UEFI moderno (sucessor do VGA texto). |
| **HAL** | Hardware Abstraction Layer — a camada do Windows NT que isola o hardware. |
