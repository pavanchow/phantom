//! The CHIP-8 CPU core: memory, registers, stack, timers, and the
//! fetch/decode/execute loop.

use crate::display::Display;
use crate::font::{FONT_SET, FONT_START};
use crate::{MEMORY_SIZE, NUM_KEYS, NUM_REGISTERS, PROGRAM_START, STACK_SIZE};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// A source of random bytes for CXNN. Boxed so tests can inject a fixed
/// sequence instead of depending on real entropy.
pub trait RandSource {
    fn next_byte(&mut self) -> u8;
}

pub struct StdRandSource {
    rng: StdRng,
}

impl StdRandSource {
    pub fn from_entropy() -> Self {
        Self {
            rng: StdRng::from_entropy(),
        }
    }

    pub fn from_seed(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }
}

impl RandSource for StdRandSource {
    fn next_byte(&mut self) -> u8 {
        self.rng.gen_range(0..=255)
    }
}

/// Test helper: always returns the same byte, or cycles through a fixed list.
pub struct FixedRandSource {
    values: Vec<u8>,
    idx: usize,
}

impl FixedRandSource {
    pub fn new(values: Vec<u8>) -> Self {
        assert!(!values.is_empty());
        Self { values, idx: 0 }
    }
}

impl RandSource for FixedRandSource {
    fn next_byte(&mut self) -> u8 {
        let v = self.values[self.idx % self.values.len()];
        self.idx += 1;
        v
    }
}

pub struct Cpu {
    pub memory: [u8; MEMORY_SIZE],
    pub v: [u8; NUM_REGISTERS],
    pub i: u16,
    pub pc: u16,
    pub stack: [u16; STACK_SIZE],
    pub sp: usize,
    pub delay_timer: u8,
    pub sound_timer: u8,
    pub keys: [bool; NUM_KEYS],
    pub display: Display,
    pub waiting_for_key: Option<u8>,
    rng: Box<dyn RandSource>,
}

impl Cpu {
    pub fn new() -> Self {
        Self::with_rand_source(Box::new(StdRandSource::from_entropy()))
    }

    pub fn with_seed(seed: u64) -> Self {
        Self::with_rand_source(Box::new(StdRandSource::from_seed(seed)))
    }

    pub fn with_rand_source(rng: Box<dyn RandSource>) -> Self {
        let mut memory = [0u8; MEMORY_SIZE];
        memory[FONT_START..FONT_START + FONT_SET.len()].copy_from_slice(&FONT_SET);
        Self {
            memory,
            v: [0; NUM_REGISTERS],
            i: 0,
            pc: PROGRAM_START,
            stack: [0; STACK_SIZE],
            sp: 0,
            delay_timer: 0,
            sound_timer: 0,
            keys: [false; NUM_KEYS],
            display: Display::new(),
            waiting_for_key: None,
            rng,
        }
    }

    pub fn load_rom(&mut self, rom: &[u8]) {
        let start = PROGRAM_START as usize;
        let end = start + rom.len();
        self.memory[start..end].copy_from_slice(rom);
    }

    /// Loads raw opcodes (already big-endian u16 pairs) at PROGRAM_START, for tests.
    pub fn load_program(&mut self, opcodes: &[u16]) {
        let mut bytes = Vec::with_capacity(opcodes.len() * 2);
        for op in opcodes {
            bytes.push((op >> 8) as u8);
            bytes.push((op & 0xFF) as u8);
        }
        self.load_rom(&bytes);
    }

    pub fn tick_timers(&mut self) {
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
    }

    fn fetch(&self) -> u16 {
        let hi = self.memory[self.pc as usize] as u16;
        let lo = self.memory[self.pc as usize + 1] as u16;
        (hi << 8) | lo
    }

    /// Fetches, decodes, and executes exactly one opcode.
    pub fn step(&mut self) {
        if self.waiting_for_key.is_some() {
            return;
        }

        let opcode = self.fetch();
        self.pc += 2;
        self.execute(opcode);
    }

    fn execute(&mut self, opcode: u16) {
        let nnn = opcode & 0x0FFF;
        let n = (opcode & 0x000F) as u8;
        let x = ((opcode & 0x0F00) >> 8) as usize;
        let y = ((opcode & 0x00F0) >> 4) as usize;
        let nn = (opcode & 0x00FF) as u8;

        match opcode & 0xF000 {
            0x0000 => match opcode {
                0x00E0 => self.display.clear(),
                0x00EE => {
                    self.sp -= 1;
                    self.pc = self.stack[self.sp];
                }
                _ => {}
            },
            0x1000 => self.pc = nnn,
            0x2000 => {
                self.stack[self.sp] = self.pc;
                self.sp += 1;
                self.pc = nnn;
            }
            0x3000 => {
                if self.v[x] == nn {
                    self.pc += 2;
                }
            }
            0x4000 => {
                if self.v[x] != nn {
                    self.pc += 2;
                }
            }
            0x5000 => {
                if n == 0 && self.v[x] == self.v[y] {
                    self.pc += 2;
                }
            }
            0x6000 => self.v[x] = nn,
            0x7000 => self.v[x] = self.v[x].wrapping_add(nn),
            0x8000 => self.execute_8xy(n, x, y),
            0x9000 => {
                if n == 0 && self.v[x] != self.v[y] {
                    self.pc += 2;
                }
            }
            0xA000 => self.i = nnn,
            0xB000 => self.pc = nnn.wrapping_add(self.v[0] as u16),
            0xC000 => {
                let r = self.rng.next_byte();
                self.v[x] = r & nn;
            }
            0xD000 => self.draw_sprite(x, y, n),
            0xE000 => match nn {
                0x9E => {
                    if self.keys[(self.v[x] & 0x0F) as usize] {
                        self.pc += 2;
                    }
                }
                0xA1 => {
                    if !self.keys[(self.v[x] & 0x0F) as usize] {
                        self.pc += 2;
                    }
                }
                _ => {}
            },
            0xF000 => self.execute_fx(x, nn),
            _ => {}
        }
    }

    fn execute_8xy(&mut self, n: u8, x: usize, y: usize) {
        match n {
            0x0 => self.v[x] = self.v[y],
            0x1 => {
                self.v[x] |= self.v[y];
                self.v[0xF] = 0;
            }
            0x2 => {
                self.v[x] &= self.v[y];
                self.v[0xF] = 0;
            }
            0x3 => {
                self.v[x] ^= self.v[y];
                self.v[0xF] = 0;
            }
            0x4 => {
                let (result, carry) = self.v[x].overflowing_add(self.v[y]);
                self.v[x] = result;
                self.v[0xF] = if carry { 1 } else { 0 };
            }
            0x5 => {
                let (result, borrow) = self.v[x].overflowing_sub(self.v[y]);
                self.v[x] = result;
                self.v[0xF] = if borrow { 0 } else { 1 };
            }
            0x6 => {
                let dropped = self.v[x] & 0x1;
                self.v[x] >>= 1;
                self.v[0xF] = dropped;
            }
            0x7 => {
                let (result, borrow) = self.v[y].overflowing_sub(self.v[x]);
                self.v[x] = result;
                self.v[0xF] = if borrow { 0 } else { 1 };
            }
            0xE => {
                let dropped = (self.v[x] & 0x80) >> 7;
                self.v[x] <<= 1;
                self.v[0xF] = dropped;
            }
            _ => {}
        }
    }

    fn execute_fx(&mut self, x: usize, nn: u8) {
        match nn {
            0x07 => self.v[x] = self.delay_timer,
            0x0A => self.waiting_for_key = Some(x as u8),
            0x15 => self.delay_timer = self.v[x],
            0x18 => self.sound_timer = self.v[x],
            0x1E => self.i = self.i.wrapping_add(self.v[x] as u16),
            0x29 => self.i = (FONT_START as u16) + (self.v[x] as u16 & 0x0F) * 5,
            0x33 => {
                let val = self.v[x];
                self.memory[self.i as usize] = val / 100;
                self.memory[self.i as usize + 1] = (val / 10) % 10;
                self.memory[self.i as usize + 2] = val % 10;
            }
            0x55 => {
                for reg in 0..=x {
                    self.memory[self.i as usize + reg] = self.v[reg];
                }
                self.i += x as u16 + 1;
            }
            0x65 => {
                for reg in 0..=x {
                    self.v[reg] = self.memory[self.i as usize + reg];
                }
                self.i += x as u16 + 1;
            }
            _ => {}
        }
    }

    fn draw_sprite(&mut self, x: usize, y: usize, n: u8) {
        let vx = self.v[x] as usize;
        let vy = self.v[y] as usize;
        let mut collision = false;
        for row in 0..n as usize {
            let byte = self.memory[self.i as usize + row];
            for col in 0..8 {
                let bit = (byte >> (7 - col)) & 1;
                if bit == 1 && self.display.xor_pixel(vx + col, vy + row, true) {
                    collision = true;
                }
            }
        }
        self.v[0xF] = if collision { 1 } else { 0 };
    }

    /// Feeds a key press into a CPU that is blocked on FX0A, resuming it.
    pub fn key_press(&mut self, key: u8) {
        self.keys[(key & 0x0F) as usize] = true;
        if let Some(vx) = self.waiting_for_key {
            self.v[vx as usize] = key;
            self.waiting_for_key = None;
        }
    }

    pub fn key_release(&mut self, key: u8) {
        self.keys[(key & 0x0F) as usize] = false;
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu() -> Cpu {
        Cpu::with_seed(1)
    }

    #[test]
    fn test_6xnn_sets_vx() {
        let mut c = cpu();
        c.load_program(&[0x62AB]);
        c.step();
        assert_eq!(c.v[2], 0xAB);
        assert_eq!(c.pc, PROGRAM_START + 2);
    }

    #[test]
    fn test_7xnn_adds_without_carry_flag() {
        let mut c = cpu();
        c.v[3] = 0x10;
        c.v[0xF] = 0x55;
        c.load_program(&[0x7305]);
        c.step();
        assert_eq!(c.v[3], 0x15);
        assert_eq!(c.v[0xF], 0x55, "7XNN must not touch VF");
    }

    #[test]
    fn test_7xnn_wraps_on_overflow() {
        let mut c = cpu();
        c.v[3] = 0xFF;
        c.load_program(&[0x7302]);
        c.step();
        assert_eq!(c.v[3], 0x01);
    }

    #[test]
    fn test_8xy4_sets_vf_carry() {
        let mut c = cpu();
        c.v[0] = 0xFF;
        c.v[1] = 0x01;
        c.load_program(&[0x8014]);
        c.step();
        assert_eq!(c.v[0], 0x00);
        assert_eq!(c.v[0xF], 1, "carry must be set on overflow");
    }

    #[test]
    fn test_8xy4_clears_vf_when_no_carry() {
        let mut c = cpu();
        c.v[0] = 0x01;
        c.v[1] = 0x01;
        c.load_program(&[0x8014]);
        c.step();
        assert_eq!(c.v[0], 0x02);
        assert_eq!(c.v[0xF], 0, "carry must be cleared without overflow");
    }

    #[test]
    fn test_8xy5_sets_vf_on_no_borrow() {
        let mut c = cpu();
        c.v[0] = 0x05;
        c.v[1] = 0x02;
        c.load_program(&[0x8015]);
        c.step();
        assert_eq!(c.v[0], 0x03);
        assert_eq!(c.v[0xF], 1, "VF=1 means no borrow occurred");
    }

    #[test]
    fn test_8xy5_clears_vf_on_borrow() {
        let mut c = cpu();
        c.v[0] = 0x01;
        c.v[1] = 0x05;
        c.load_program(&[0x8015]);
        c.step();
        assert_eq!(c.v[0], 0xFC);
        assert_eq!(c.v[0xF], 0, "VF=0 means a borrow occurred");
    }

    #[test]
    fn test_8xy6_shift_right_sets_vf_to_dropped_bit() {
        let mut c = cpu();
        c.v[0] = 0b0000_0011;
        c.load_program(&[0x8016]);
        c.step();
        assert_eq!(c.v[0], 0b0000_0001);
        assert_eq!(c.v[0xF], 1);
    }

    #[test]
    fn test_8xye_shift_left_sets_vf_to_dropped_bit() {
        let mut c = cpu();
        c.v[0] = 0b1000_0001;
        c.load_program(&[0x801E]);
        c.step();
        assert_eq!(c.v[0], 0b0000_0010);
        assert_eq!(c.v[0xF], 1);
    }

    #[test]
    fn test_annn_sets_i() {
        let mut c = cpu();
        c.load_program(&[0xA123]);
        c.step();
        assert_eq!(c.i, 0x123);
    }

    #[test]
    fn test_3xnn_skips_when_equal() {
        let mut c = cpu();
        c.v[0] = 0x42;
        c.load_program(&[0x3042, 0x6001, 0x6002]);
        c.step();
        assert_eq!(c.pc, PROGRAM_START + 4, "should skip the 6001 instruction");
        c.step();
        assert_eq!(c.v[0], 0x02);
    }

    #[test]
    fn test_3xnn_does_not_skip_when_not_equal() {
        let mut c = cpu();
        c.v[0] = 0x01;
        c.load_program(&[0x3042, 0x6009]);
        c.step();
        assert_eq!(c.pc, PROGRAM_START + 2);
    }

    #[test]
    fn test_4xnn_skips_when_not_equal() {
        let mut c = cpu();
        c.v[0] = 0x01;
        c.load_program(&[0x4042]);
        c.step();
        assert_eq!(c.pc, PROGRAM_START + 4);
    }

    #[test]
    fn test_5xy0_skips_when_equal() {
        let mut c = cpu();
        c.v[0] = 5;
        c.v[1] = 5;
        c.load_program(&[0x5010]);
        c.step();
        assert_eq!(c.pc, PROGRAM_START + 4);
    }

    #[test]
    fn test_9xy0_skips_when_not_equal() {
        let mut c = cpu();
        c.v[0] = 5;
        c.v[1] = 6;
        c.load_program(&[0x9010]);
        c.step();
        assert_eq!(c.pc, PROGRAM_START + 4);
    }

    #[test]
    fn test_1nnn_jump() {
        let mut c = cpu();
        c.load_program(&[0x1300]);
        c.step();
        assert_eq!(c.pc, 0x300);
    }

    #[test]
    fn test_2nnn_call_and_00ee_return() {
        let mut c = cpu();
        c.load_program(&[0x2300]);
        c.step();
        assert_eq!(c.pc, 0x300);
        assert_eq!(c.sp, 1);
        assert_eq!(c.stack[0], PROGRAM_START + 2);

        c.memory[0x300] = 0x00;
        c.memory[0x301] = 0xEE;
        c.step();
        assert_eq!(c.pc, PROGRAM_START + 2);
        assert_eq!(c.sp, 0);
    }

    #[test]
    fn test_bnnn_jump_plus_v0() {
        let mut c = cpu();
        c.v[0] = 0x05;
        c.load_program(&[0xB300]);
        c.step();
        assert_eq!(c.pc, 0x305);
    }

    #[test]
    fn test_cxnn_masks_random_byte() {
        let mut c = Cpu::with_rand_source(Box::new(FixedRandSource::new(vec![0xFF])));
        c.load_program(&[0xC00F]);
        c.step();
        assert_eq!(c.v[0], 0x0F, "random byte must be masked by NN");
    }

    #[test]
    fn test_00e0_clears_display() {
        let mut c = cpu();
        c.display.xor_pixel(0, 0, true);
        assert!(c.display.get(0, 0));
        c.load_program(&[0x00E0]);
        c.step();
        assert!(!c.display.get(0, 0));
    }

    #[test]
    fn test_dxyn_draws_sprite_and_sets_vf_on_collision() {
        let mut c = cpu();
        // I -> font digit 0 (5-byte sprite), drawn twice at the same spot.
        c.v[0xA] = 0;
        c.load_program(&[0xF029, 0xD005, 0xD005]);
        c.step(); // FX29: I = font sprite for digit 0
        c.step(); // first draw: no collision yet
        assert_eq!(c.v[0xF], 0, "first draw should not collide");
        assert!(c.display.get(0, 0), "top-left pixel of '0' glyph should be set");
        c.step(); // second draw over same pixels: full collision, erases them
        assert_eq!(c.v[0xF], 1, "redrawing the same sprite must set VF on collision");
        assert!(!c.display.get(0, 0), "XOR draw should have erased the pixel");
    }

    #[test]
    fn test_dxyn_wraps_around_edges() {
        let mut c = cpu();
        c.v[0] = 63; // x
        c.v[1] = 0; // y
        c.i = 0x300;
        c.memory[0x300] = 0b1100_0000;
        c.load_program(&[0xD011]);
        c.step();
        assert!(c.display.get(63, 0));
        assert!(c.display.get(0, 0), "sprite must wrap past the right edge");
    }

    #[test]
    fn test_ex9e_skips_when_key_pressed() {
        let mut c = cpu();
        c.v[0] = 3;
        c.keys[3] = true;
        c.load_program(&[0xE09E]);
        c.step();
        assert_eq!(c.pc, PROGRAM_START + 4);
    }

    #[test]
    fn test_exa1_skips_when_key_not_pressed() {
        let mut c = cpu();
        c.v[0] = 3;
        c.load_program(&[0xE0A1]);
        c.step();
        assert_eq!(c.pc, PROGRAM_START + 4);
    }

    #[test]
    fn test_fx07_and_fx15_timer_roundtrip() {
        let mut c = cpu();
        c.v[0] = 42;
        c.load_program(&[0xF015, 0xF007]);
        c.step();
        assert_eq!(c.delay_timer, 42);
        c.step();
        assert_eq!(c.v[0], 42);
    }

    #[test]
    fn test_fx0a_blocks_until_key_press() {
        let mut c = cpu();
        c.load_program(&[0xF00A, 0x6199]);
        c.step();
        assert!(c.waiting_for_key.is_some());
        let pc_before = c.pc;
        c.step(); // should be a no-op, still waiting
        assert_eq!(c.pc, pc_before);
        c.key_press(7);
        assert_eq!(c.v[0], 7);
        assert!(c.waiting_for_key.is_none());
        c.step();
        assert_eq!(c.v[1], 0x99);
    }

    #[test]
    fn test_fx1e_adds_to_i() {
        let mut c = cpu();
        c.v[0] = 0x10;
        c.i = 0x200;
        c.load_program(&[0xF01E]);
        c.step();
        assert_eq!(c.i, 0x210);
    }

    #[test]
    fn test_fx29_sets_i_to_font_glyph() {
        let mut c = cpu();
        c.v[0] = 0xA;
        c.load_program(&[0xF029]);
        c.step();
        assert_eq!(c.i, (FONT_START as u16) + 0xA * 5);
    }

    #[test]
    fn test_fx33_writes_bcd_digits() {
        let mut c = cpu();
        c.v[0] = 195;
        c.i = 0x300;
        c.load_program(&[0xF033]);
        c.step();
        assert_eq!(c.memory[0x300], 1);
        assert_eq!(c.memory[0x301], 9);
        assert_eq!(c.memory[0x302], 5);
    }

    #[test]
    fn test_fx33_zero() {
        let mut c = cpu();
        c.v[0] = 0;
        c.i = 0x300;
        c.load_program(&[0xF033]);
        c.step();
        assert_eq!(c.memory[0x300], 0);
        assert_eq!(c.memory[0x301], 0);
        assert_eq!(c.memory[0x302], 0);
    }

    #[test]
    fn test_fx55_stores_registers_to_memory() {
        let mut c = cpu();
        c.v[0] = 1;
        c.v[1] = 2;
        c.v[2] = 3;
        c.i = 0x300;
        c.load_program(&[0xF255]);
        c.step();
        assert_eq!(c.memory[0x300], 1);
        assert_eq!(c.memory[0x301], 2);
        assert_eq!(c.memory[0x302], 3);
        assert_eq!(c.i, 0x303, "I should advance past the stored registers");
    }

    #[test]
    fn test_fx65_loads_registers_from_memory() {
        let mut c = cpu();
        c.i = 0x300;
        c.memory[0x300] = 9;
        c.memory[0x301] = 8;
        c.memory[0x302] = 7;
        c.load_program(&[0xF265]);
        c.step();
        assert_eq!(c.v[0], 9);
        assert_eq!(c.v[1], 8);
        assert_eq!(c.v[2], 7);
        assert_eq!(c.i, 0x303);
    }

    #[test]
    fn test_small_program_runs_n_steps_and_reaches_expected_state() {
        let mut c = cpu();
        // V0 = 5, V1 = 10, V0 += V1, I = 0x400, skip if V0 == 15, V2 = 0xAA (skipped)
        c.load_program(&[
            0x6005, // V0 = 5
            0x610A, // V1 = 10
            0x8014, // V0 += V1 (VF carry)
            0xA400, // I = 0x400
            0x300F, // skip next if V0 == 15
            0x62AA, // V2 = 0xAA (should be skipped)
            0x63BB, // V3 = 0xBB (should execute)
        ]);
        for _ in 0..6 {
            c.step();
        }
        assert_eq!(c.v[0], 15);
        assert_eq!(c.v[1], 10);
        assert_eq!(c.v[0xF], 0);
        assert_eq!(c.i, 0x400);
        assert_eq!(c.v[2], 0, "V2 write should have been skipped");
        assert_eq!(c.v[3], 0xBB);
    }

    #[test]
    fn test_tick_timers_decrements_and_floors_at_zero() {
        let mut c = cpu();
        c.delay_timer = 1;
        c.sound_timer = 0;
        c.tick_timers();
        assert_eq!(c.delay_timer, 0);
        assert_eq!(c.sound_timer, 0);
        c.tick_timers();
        assert_eq!(c.delay_timer, 0);
    }

    #[test]
    fn test_font_loaded_at_start() {
        let c = cpu();
        assert_eq!(
            &c.memory[FONT_START..FONT_START + 5],
            &FONT_SET[0..5],
            "digit 0 glyph should be loaded into memory"
        );
    }
}
