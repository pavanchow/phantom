//! Phantom: a from-scratch CHIP-8 CPU emulator core.
//!
//! The core is headless and deterministic: `Cpu::step()` fetches, decodes,
//! and executes exactly one opcode, with no I/O or timing baked in. That
//! makes every instruction testable in isolation.

pub mod cpu;
pub mod display;
pub mod font;

pub use cpu::Cpu;
pub use display::{Display, DISPLAY_HEIGHT, DISPLAY_WIDTH};

pub const MEMORY_SIZE: usize = 4096;
pub const NUM_REGISTERS: usize = 16;
pub const STACK_SIZE: usize = 16;
pub const NUM_KEYS: usize = 16;
pub const PROGRAM_START: u16 = 0x200;
