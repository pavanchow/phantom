# Design

## The machine model

CHIP-8 is a simple virtual machine from the 1970s, originally built so hobbyists could write games in a compact bytecode instead of raw machine code. Phantom models it as:

- **Memory**: 4096 bytes. Programs load at `0x200`. The interpreter itself would traditionally occupy `0x000`-`0x1FF`; Phantom uses that space to hold the built-in font set starting at `0x50`.
- **Registers**: 16 general-purpose 8-bit registers, `V0`-`VF`. `VF` doubles as a flag register, set by several opcodes (carry, borrow, collision, shift-out bit).
- **I register**: a 16-bit address register, used by sprite draws, register dump/load, and BCD conversion.
- **Program counter**: 16-bit, advances by 2 after each fetch (all opcodes are 2 bytes).
- **Stack**: 16 levels deep, used only for subroutine call/return (`2NNN` / `00EE`).
- **Timers**: an 8-bit delay timer and an 8-bit sound timer, both decrementing at a fixed rate independent of CPU speed. Phantom exposes `tick_timers()` as a separate call from `step()` so a caller controls the ratio between CPU steps and timer ticks.
- **Display**: a 64x32 monochrome buffer. Sprites are drawn by XOR-ing pixels on, which is what makes collision detection possible: XOR-ing a set pixel back off is what the spec defines as a collision.
- **Keypad**: 16 keys (`0x0`-`0xF`), tracked as a boolean array.

## The fetch/decode/execute loop

`Cpu::step()` does exactly three things: read two bytes at `pc` into a 16-bit opcode, advance `pc` by 2, and dispatch on the opcode's high nibble (with sub-dispatch on the low nibble for `0x8` and on the low byte for `0xE`/`0xF`). It does not sleep, does not touch a display buffer besides the CHIP-8 display, and does not read real time. That is the entire design decision that makes the core testable: nothing about `step()` depends on wall-clock time or on being driven by any particular front end.

`FX0A` (wait for keypress) is the one instruction that blocks. Rather than spinning inside `step()`, Phantom sets `waiting_for_key` to the destination register and makes subsequent `step()` calls a no-op until `key_press()` is called externally. This keeps `step()` non-blocking and lets a front end keep polling input and redrawing while a ROM waits on a key.

## Display and collision

The display stores a flat `bool` array. `xor_pixel(x, y, on)` wraps `x` and `y` modulo the display dimensions (CHIP-8 sprites wrap at the edges) and returns whether the XOR flipped a previously-set pixel off. `DXYN` draws each row of the sprite (from memory starting at `I`) bit by bit, ORs the per-pixel collision results together, and writes the final result into `VF`. This is the one opcode with genuinely stateful, order-dependent behavior, so it gets its own two tests: a wraparound case and a collision case that draws the same sprite twice and checks the pixel got erased with VF set.

## Timers

Timers are deliberately decoupled from `step()`. Real CHIP-8 hardware runs the CPU at a few hundred to a thousand instructions per second and decrements both timers at a fixed 60 Hz regardless of CPU speed. Phantom's CLI runner calls `step()` some number of times per frame (`--speed`, default 9) and then calls `tick_timers()` once per frame, approximating that ratio without hardcoding a CPU clock speed into the core.

## Keeping the core testable

Two design choices exist purely so tests can assert exact behavior:

1. **No wall-clock or real I/O in `step()`.** A test can call `step()` in a tight loop and get fully deterministic results.
2. **Injectable randomness.** `CXNN` (`VX = random & NN`) goes through a `RandSource` trait object instead of calling `rand::random()` directly. Production code uses `StdRandSource` (seedable, backed by `rand`'s `StdRng`); tests use `FixedRandSource`, which cycles through a fixed list of bytes, so a test can assert the exact masked result without depending on entropy.

Everything above the core, the CLI parsing (`clap`) and the ANSI terminal renderer, is a thin consumer of the `Cpu` struct and has no tests of its own; it doesn't need them, because it contains no CHIP-8 semantics.
