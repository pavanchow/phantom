<img src="docs/logo.svg" alt="Phantom logo" width="96">

# Phantom: a CHIP-8 emulator in Rust

Phantom is a CHIP-8 emulator written from scratch in Rust where the CPU is the star, not the graphics. The core is a headless, deterministic, fully unit-tested state machine you can step one opcode at a time and assert exact register, memory, and display state on, covering the full standard opcode set including carry, borrow, and sprite collision. A thin terminal renderer sits on top so you can watch a ROM run, but the CPU core has no dependency on it.

**[Live demo](https://pavanchow.github.io/phantom/)** · MIT licensed · written in Rust

## The tested-core angle

`Cpu::step()` fetches, decodes, and executes exactly one opcode with no I/O and no timing baked in. That makes every instruction independently testable:

- Load a raw opcode (or a short program) straight into memory
- Call `step()` some number of times
- Assert exact `v`, `i`, `pc`, `stack`, memory, and display state

Randomness (`CXNN`) goes through an injectable `RandSource` trait, so tests use a `FixedRandSource` that returns a known sequence instead of depending on real entropy. No flaky tests, no `sleep`, no screen-scraping to verify behavior.

## Opcode coverage

All standard CHIP-8 opcodes are implemented and unit tested individually:

- `00E0` clear, `00EE` return, `1NNN` jump, `2NNN` call
- `3XNN` / `4XNN` / `5XY0` / `9XY0` conditional skips
- `6XNN` set, `7XNN` add (wrapping, no carry flag touched)
- `8XY0`-`8XYE`: assign, OR, AND, XOR, add-with-carry, sub-with-borrow, shift-right, sub-with-borrow (reverse), shift-left
- `ANNN` set I, `BNNN` jump + V0, `CXNN` masked random
- `DXYN` draw sprite with wraparound and VF collision flag
- `EX9E` / `EXA1` key-state skips
- `FX07`/`FX0A`/`FX15`/`FX18`/`FX1E`/`FX29`/`FX33`/`FX55`/`FX65`

Carry and borrow flags on `8XY4`/`8XY5` are asserted exactly, both the set and cleared case. `DXYN` collision detection is tested by drawing the same sprite twice and confirming VF flips.

## Usage

```
cargo build --release
cargo run --release -- run path/to/rom.ch8
cargo run --release -- run              # runs a bundled demo ROM
```

The terminal renderer draws the 64x32 display with ANSI escape codes, redrawing each frame, and ticks the delay and sound timers alongside CPU steps.

## Tests

```
cargo test
```

35 tests covering every opcode, the carry/borrow flag cases, sprite collision, and a multi-instruction program run that asserts final CPU state.

## Project layout

- `src/cpu.rs` - the CHIP-8 core: memory, registers, stack, timers, fetch/decode/execute
- `src/display.rs` - the 64x32 monochrome pixel buffer and XOR-draw logic
- `src/font.rs` - the standard CHIP-8 font set
- `src/main.rs` - the CLI and terminal renderer

## License

MIT. By Pavan Nallamothu.
