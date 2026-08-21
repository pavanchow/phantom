use clap::{Parser, Subcommand};
use phantom::{Cpu, DISPLAY_HEIGHT, DISPLAY_WIDTH};
use std::fs;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "phantom", about = "A CHIP-8 CPU emulator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a CHIP-8 ROM, rendering the display to the terminal.
    Run {
        /// Path to a .ch8 ROM file. Omit to run the bundled demo ROM.
        rom: Option<String>,
        /// Instructions executed per frame.
        #[arg(long, default_value_t = 9)]
        speed: u32,
    },
}

/// A tiny bundled demo: draws a bouncing pattern using the font digits
/// so `phantom run` has something to show without an external ROM file.
fn demo_rom() -> Vec<u8> {
    vec![
        0x60, 0x05, // V0 = 5   (x)
        0x61, 0x05, // V1 = 5   (y)
        0x64, 0x00, // V4 = 0   (font digit index)
        0xF4, 0x29, // I = font glyph for digit 0
        0xD0, 0x15, // draw 5-row sprite at (V0, V1)   <- loop target, 0x208
        0x00, 0xE0, // clear
        0x70, 0x01, // V0 += 1  (wraps at the display edge)
        0x71, 0x01, // V1 += 1
        0x12, 0x08, // jump back to the draw instruction
    ]
}

fn render(cpu: &Cpu) {
    print!("\x1B[H"); // cursor home, avoid full clear flicker
    let mut out = String::with_capacity((DISPLAY_WIDTH + 1) * DISPLAY_HEIGHT);
    for y in 0..DISPLAY_HEIGHT {
        for x in 0..DISPLAY_WIDTH {
            out.push(if cpu.display.get(x, y) { '#' } else { ' ' });
        }
        out.push('\n');
    }
    print!("{out}");
    std::io::stdout().flush().ok();
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Run { rom, speed } => {
            let rom_bytes = match rom {
                Some(path) => fs::read(&path).unwrap_or_else(|e| {
                    eprintln!("failed to read {path}: {e}");
                    std::process::exit(1);
                }),
                None => demo_rom(),
            };

            let mut cpu = Cpu::new();
            cpu.load_rom(&rom_bytes);

            print!("\x1B[2J"); // clear once up front
            for _ in 0..600 {
                for _ in 0..speed {
                    cpu.step();
                }
                cpu.tick_timers();
                render(&cpu);
                sleep(Duration::from_millis(16));
            }
        }
    }
}
