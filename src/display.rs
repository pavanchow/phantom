//! The 64x32 monochrome display buffer.

pub const DISPLAY_WIDTH: usize = 64;
pub const DISPLAY_HEIGHT: usize = 32;

#[derive(Clone)]
pub struct Display {
    pixels: [bool; DISPLAY_WIDTH * DISPLAY_HEIGHT],
}

impl Default for Display {
    fn default() -> Self {
        Self {
            pixels: [false; DISPLAY_WIDTH * DISPLAY_HEIGHT],
        }
    }
}

impl Display {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.pixels = [false; DISPLAY_WIDTH * DISPLAY_HEIGHT];
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        self.pixels[(y % DISPLAY_HEIGHT) * DISPLAY_WIDTH + (x % DISPLAY_WIDTH)]
    }

    /// XORs a single pixel on, wrapping at the display edges.
    /// Returns true if this flipped a set pixel off (a collision).
    pub fn xor_pixel(&mut self, x: usize, y: usize, on: bool) -> bool {
        let idx = (y % DISPLAY_HEIGHT) * DISPLAY_WIDTH + (x % DISPLAY_WIDTH);
        let was_set = self.pixels[idx];
        let new_val = self.pixels[idx] ^ on;
        self.pixels[idx] = new_val;
        was_set && on
    }

    pub fn pixels(&self) -> &[bool] {
        &self.pixels
    }
}
