//! Simple text rendering for overlays and default screen

const FONT_WIDTH: usize = 8;
const FONT_HEIGHT: usize = 8;

/// Simple 8x8 bitmap font (subset of ASCII printable characters)
/// Each character is represented as 8 bytes (one per row)
fn get_char_bitmap(c: char) -> [u8; 8] {
    match c {
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '!' => [0x18, 0x18, 0x18, 0x18, 0x00, 0x00, 0x18, 0x00],
        '(' => [0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00],
        ')' => [0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00],
        '+' => [0x00, 0x18, 0x18, 0x7E, 0x18, 0x18, 0x00, 0x00],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30],
        '-' => [0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
        '/' => [0x00, 0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x00],
        '0' => [0x3C, 0x66, 0x6E, 0x7E, 0x76, 0x66, 0x3C, 0x00],
        '1' => [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
        '2' => [0x3C, 0x66, 0x06, 0x0C, 0x18, 0x30, 0x7E, 0x00],
        '3' => [0x3C, 0x66, 0x06, 0x1C, 0x06, 0x66, 0x3C, 0x00],
        '4' => [0x0C, 0x1C, 0x3C, 0x6C, 0x7E, 0x0C, 0x0C, 0x00],
        '5' => [0x7E, 0x60, 0x7C, 0x06, 0x06, 0x66, 0x3C, 0x00],
        '6' => [0x1C, 0x30, 0x60, 0x7C, 0x66, 0x66, 0x3C, 0x00],
        '7' => [0x7E, 0x06, 0x0C, 0x18, 0x30, 0x30, 0x30, 0x00],
        '8' => [0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00],
        '9' => [0x3C, 0x66, 0x66, 0x3E, 0x06, 0x0C, 0x38, 0x00],
        ':' => [0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00],
        '<' => [0x06, 0x0C, 0x18, 0x30, 0x18, 0x0C, 0x06, 0x00],
        '=' => [0x00, 0x00, 0x7E, 0x00, 0x7E, 0x00, 0x00, 0x00],
        '>' => [0x60, 0x30, 0x18, 0x0C, 0x18, 0x30, 0x60, 0x00],
        'A' => [0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00],
        'B' => [0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x7C, 0x00],
        'C' => [0x3C, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3C, 0x00],
        'D' => [0x78, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0x78, 0x00],
        'E' => [0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x7E, 0x00],
        'F' => [0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x60, 0x00],
        'G' => [0x3C, 0x66, 0x60, 0x6E, 0x66, 0x66, 0x3C, 0x00],
        'H' => [0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00],
        'I' => [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
        'J' => [0x3E, 0x0C, 0x0C, 0x0C, 0x0C, 0x6C, 0x38, 0x00],
        'K' => [0x66, 0x6C, 0x78, 0x70, 0x78, 0x6C, 0x66, 0x00],
        'L' => [0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7E, 0x00],
        'M' => [0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63, 0x63, 0x00],
        'N' => [0x66, 0x76, 0x7E, 0x6E, 0x66, 0x66, 0x66, 0x00],
        'O' => [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
        'P' => [0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0x00],
        'Q' => [0x3C, 0x66, 0x66, 0x66, 0x6A, 0x6C, 0x36, 0x00],
        'R' => [0x7C, 0x66, 0x66, 0x7C, 0x6C, 0x66, 0x66, 0x00],
        'S' => [0x3C, 0x66, 0x60, 0x3C, 0x06, 0x66, 0x3C, 0x00],
        'T' => [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00],
        'U' => [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00],
        'V' => [0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00],
        'W' => [0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00],
        'X' => [0x66, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x66, 0x00],
        'Y' => [0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x00],
        'Z' => [0x7E, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x7E, 0x00],
        'a' => [0x00, 0x00, 0x3C, 0x06, 0x3E, 0x66, 0x3E, 0x00],
        'b' => [0x60, 0x60, 0x7C, 0x66, 0x66, 0x66, 0x7C, 0x00],
        'c' => [0x00, 0x00, 0x3C, 0x66, 0x60, 0x66, 0x3C, 0x00],
        'd' => [0x06, 0x06, 0x3E, 0x66, 0x66, 0x66, 0x3E, 0x00],
        'e' => [0x00, 0x00, 0x3C, 0x66, 0x7E, 0x60, 0x3C, 0x00],
        'f' => [0x1C, 0x30, 0x30, 0x7C, 0x30, 0x30, 0x30, 0x00],
        'g' => [0x00, 0x00, 0x3E, 0x66, 0x66, 0x3E, 0x06, 0x3C],
        'h' => [0x60, 0x60, 0x7C, 0x66, 0x66, 0x66, 0x66, 0x00],
        'i' => [0x18, 0x00, 0x38, 0x18, 0x18, 0x18, 0x3C, 0x00],
        'j' => [0x0C, 0x00, 0x1C, 0x0C, 0x0C, 0x0C, 0x6C, 0x38],
        'k' => [0x60, 0x60, 0x66, 0x6C, 0x78, 0x6C, 0x66, 0x00],
        'l' => [0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00],
        'm' => [0x00, 0x00, 0x66, 0x7F, 0x6B, 0x6B, 0x63, 0x00],
        'n' => [0x00, 0x00, 0x7C, 0x66, 0x66, 0x66, 0x66, 0x00],
        'o' => [0x00, 0x00, 0x3C, 0x66, 0x66, 0x66, 0x3C, 0x00],
        'p' => [0x00, 0x00, 0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60],
        'q' => [0x00, 0x00, 0x3E, 0x66, 0x66, 0x3E, 0x06, 0x06],
        'r' => [0x00, 0x00, 0x6C, 0x76, 0x60, 0x60, 0x60, 0x00],
        's' => [0x00, 0x00, 0x3E, 0x60, 0x3C, 0x06, 0x7C, 0x00],
        't' => [0x30, 0x30, 0x7C, 0x30, 0x30, 0x30, 0x1C, 0x00],
        'u' => [0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x3E, 0x00],
        'v' => [0x00, 0x00, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00],
        'w' => [0x00, 0x00, 0x63, 0x6B, 0x6B, 0x7F, 0x36, 0x00],
        'x' => [0x00, 0x00, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x00],
        'y' => [0x00, 0x00, 0x66, 0x66, 0x66, 0x3E, 0x06, 0x3C],
        'z' => [0x00, 0x00, 0x7E, 0x0C, 0x18, 0x30, 0x7E, 0x00],
        _ => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    }
}

/// Draw a string on a framebuffer
#[allow(clippy::too_many_arguments)]
pub fn draw_text(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    text: &str,
    x: usize,
    y: usize,
    color: u32,
) {
    let mut cursor_x = x;
    let cursor_y = y;

    for c in text.chars() {
        if c == '\n' {
            // Newlines not supported in this simple implementation
            continue;
        }

        let bitmap = get_char_bitmap(c);

        for (row, &bitmap_row) in bitmap.iter().enumerate().take(FONT_HEIGHT) {
            for col in 0..FONT_WIDTH {
                if cursor_y + row >= height || cursor_x + col >= width {
                    continue;
                }

                let bit = (bitmap_row >> (7 - col)) & 1;
                if bit == 1 {
                    let idx = (cursor_y + row) * width + cursor_x + col;
                    if idx < buffer.len() {
                        buffer[idx] = color;
                    }
                }
            }
        }

        cursor_x += FONT_WIDTH;
        if cursor_x >= width {
            break;
        }
    }
}

/// Draw multiple lines of text
#[allow(clippy::too_many_arguments)]
pub fn draw_text_lines(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    lines: &[&str],
    start_x: usize,
    start_y: usize,
    line_spacing: usize,
    color: u32,
) {
    for (i, line) in lines.iter().enumerate() {
        let y = start_y + i * line_spacing;
        if y + FONT_HEIGHT > height {
            break;
        }
        draw_text(buffer, width, height, line, start_x, y, color);
    }
}

/// Create the default splash screen
#[allow(dead_code)]
pub fn create_default_screen(width: usize, height: usize) -> Vec<u32> {
    let mut buffer = vec![0xFF1A1A2E; width * height]; // Dark blue background

    // Hemulator logo/title - draw each line centered individually
    let logo_y = height / 3;

    // Center "HEMULATOR" (9 characters)
    let hemulator_x = (width - 9 * FONT_WIDTH) / 2;
    draw_text(
        &mut buffer,
        width,
        height,
        "HEMULATOR",
        hemulator_x,
        logo_y,
        0xFF16F2B3,
    );

    // Center "Multi-System Emulator" (21 characters)
    let subtitle_x = (width - 21 * FONT_WIDTH) / 2;
    draw_text(
        &mut buffer,
        width,
        height,
        "Multi-System Emulator",
        subtitle_x,
        logo_y + (FONT_HEIGHT + 4) * 2,
        0xFF16F2B3,
    );

    // Instructions - center each line individually
    let inst_y = height * 2 / 3;

    // "Press F3 to open a ROM" (22 characters)
    let inst1_x = (width - 22 * FONT_WIDTH) / 2;
    draw_text(
        &mut buffer,
        width,
        height,
        "Press F3 to open a ROM",
        inst1_x,
        inst_y,
        0xFFF0F0F0,
    );

    // "Press F1 for help" (17 characters)
    let inst2_x = (width - 17 * FONT_WIDTH) / 2;
    draw_text(
        &mut buffer,
        width,
        height,
        "Press F1 for help",
        inst2_x,
        inst_y + FONT_HEIGHT + 4,
        0xFFF0F0F0,
    );

    buffer
}

/// Create a help overlay
pub fn create_help_overlay(
    width: usize,
    height: usize,
    settings: &crate::settings::Settings,
) -> Vec<u32> {
    // Semi-transparent dark background
    let mut buffer = vec![0xC0000000; width * height];

    // Player 1 controls
    let p1_a = format!("  {} - A", settings.input.player1.a);
    let p1_b = format!("  {} - B", settings.input.player1.b);
    let p1_select = format!("  {} - Select", settings.input.player1.select);
    let p1_start = format!("  {} - Start", settings.input.player1.start);
    let p1_dpad = format!(
        "  {} {} {} {} - D-pad",
        settings.input.player1.up,
        settings.input.player1.down,
        settings.input.player1.left,
        settings.input.player1.right
    );

    // Player 2 controls (if mapped)
    let p2_mapped = !settings.input.player2.a.is_empty();
    let p2_a = format!("  {} - A", settings.input.player2.a);
    let p2_b = format!("  {} - B", settings.input.player2.b);
    let p2_dpad = format!(
        "  {} {} {} {} - D-pad",
        settings.input.player2.up,
        settings.input.player2.down,
        settings.input.player2.left,
        settings.input.player2.right
    );

    let mut help_lines: Vec<&str> = vec![
        "HEMULATOR - Help",
        "",
        "Player 1 Controller:",
        &p1_a,
        &p1_b,
        &p1_select,
        &p1_start,
        &p1_dpad,
    ];

    if p2_mapped {
        help_lines.push("");
        help_lines.push("Player 2 Controller:");
        help_lines.push(&p2_a);
        help_lines.push(&p2_b);
        help_lines.push(&p2_dpad);
    }

    help_lines.extend_from_slice(&[
        "",
        "HOST KEY: Hold Right Alt for emulator controls",
        "",
        "Function Keys (with Right Alt):",
        "  F1  - Help",
        "  F2  - Speed",
        "  F3  - Mount points",
        "  F4  - Screenshot",
        "  F5  - Save state",
        "  F6  - Load state",
        "  F7  - Load project",
        "  F8  - Save project",
        "  F10 - Debug info",
        "  F11 - CRT filter",
        "  F12 - Reset",
        "  ESC - Exit emulator",
        "",
        "Without Right Ctrl: All keys pass to DOS",
        "(ESC works in DOS, BIOS, etc.)",
        "",
        "Press F1 to close",
    ]);

    let start_x = 10;
    let start_y = 10;

    draw_text_lines(
        &mut buffer,
        width,
        height,
        &help_lines,
        start_x,
        start_y,
        FONT_HEIGHT + 1, // Reduced from +2 to +1 for tighter spacing
        0xFFFFFFFF,
    );

    buffer
}

/// Create a slot selection overlay for save/load
#[allow(clippy::too_many_arguments)]
pub fn create_slot_selector_overlay(
    width: usize,
    height: usize,
    mode: &str, // "SAVE" or "LOAD"
    has_saves: &[bool; 5],
) -> Vec<u32> {
    // Semi-transparent dark background
    let mut buffer = vec![0xC0000000; width * height];

    let title = if mode == "SAVE" {
        "SAVE STATE - Select Slot (1-5)"
    } else {
        "LOAD STATE - Select Slot (1-5)"
    };

    let mut all_lines = Vec::new();
    all_lines.push(title);
    all_lines.push("");

    // Prepare slot lines
    let slot1 = if mode == "LOAD" && !has_saves[0] {
        "  1 - Slot 1 (empty)"
    } else {
        "  1 - Slot 1"
    };
    let slot2 = if mode == "LOAD" && !has_saves[1] {
        "  2 - Slot 2 (empty)"
    } else {
        "  2 - Slot 2"
    };
    let slot3 = if mode == "LOAD" && !has_saves[2] {
        "  3 - Slot 3 (empty)"
    } else {
        "  3 - Slot 3"
    };
    let slot4 = if mode == "LOAD" && !has_saves[3] {
        "  4 - Slot 4 (empty)"
    } else {
        "  4 - Slot 4"
    };
    let slot5 = if mode == "LOAD" && !has_saves[4] {
        "  5 - Slot 5 (empty)"
    } else {
        "  5 - Slot 5"
    };

    all_lines.push(slot1);
    all_lines.push(slot2);
    all_lines.push(slot3);
    all_lines.push(slot4);
    all_lines.push(slot5);
    all_lines.push("");
    all_lines.push("Press 1-5 to select, ESC to cancel");

    let start_x = (width.saturating_sub(30 * FONT_WIDTH)) / 2;
    let start_y = height / 3;

    draw_text_lines(
        &mut buffer,
        width,
        height,
        &all_lines,
        start_x,
        start_y,
        FONT_HEIGHT + 2,
        0xFFFFFFFF,
    );

    buffer
}

/// Create a debug info overlay
#[allow(clippy::too_many_arguments)]
pub fn create_debug_overlay(
    width: usize,
    height: usize,
    mapper_name: &str,
    mapper_number: u8,
    timing_mode: &str,
    prg_banks: usize,
    chr_banks: usize,
    fps: f64,
    runtime: emu_nes::RuntimeStats,
    video_backend: &str,
) -> Vec<u32> {
    // Semi-transparent dark background
    let mut buffer = vec![0xC0000000; width * height];

    let prg_line = format!("PRG: {} x 16KB", prg_banks);
    let chr_line = if chr_banks == 0 {
        "CHR: RAM".to_string()
    } else {
        format!("CHR: {} x 8KB", chr_banks)
    };
    let mapper_line = format!("Mapper: {} ({})", mapper_number, mapper_name);
    let timing_line = format!("Timing: {}", timing_mode);
    let fps_line = format!("FPS: {:.1}", fps);
    let video_line = format!("Video: {}", video_backend);

    let pc_line = format!("PC: 0x{:04X}", runtime.pc);
    let vec_line = format!(
        "VEC: reset=0x{:04X} nmi=0x{:04X} irq=0x{:04X}",
        runtime.vec_reset, runtime.vec_nmi, runtime.vec_irq
    );
    let hot0 = runtime.pc_hotspots[0];
    let hot1 = runtime.pc_hotspots[1];
    let hot2 = runtime.pc_hotspots[2];
    let pc_hot_line = format!(
        "PC hot: [{:04X} x{}] [{:04X} x{}] [{:04X} x{}]",
        hot0.pc, hot0.count, hot1.pc, hot1.count, hot2.pc, hot2.count
    );
    let cpu_line = format!(
        "CPU: steps={} cycles={}",
        runtime.cpu_steps, runtime.cpu_cycles
    );
    let int_line = format!(
        "INT: irq={} nmi={} a12_edges={}",
        runtime.irqs, runtime.nmis, runtime.mmc3_a12_edges
    );
    let ppu_line = format!(
        "PPU: ctrl=0x{:02X} mask=0x{:02X} vblank={}",
        runtime.ppu_ctrl,
        runtime.ppu_mask,
        if runtime.ppu_vblank { "1" } else { "0" }
    );

    let debug_lines: Vec<&str> = vec![
        "DEBUG INFO",
        "",
        &mapper_line,
        &prg_line,
        &chr_line,
        &timing_line,
        &fps_line,
        &video_line,
        "",
        &pc_line,
        &vec_line,
        &pc_hot_line,
        &cpu_line,
        &int_line,
        &ppu_line,
        "",
        "Press F10 to close",
    ];

    let start_x = 10;
    let start_y = 10;

    draw_text_lines(
        &mut buffer,
        width,
        height,
        &debug_lines,
        start_x,
        start_y,
        FONT_HEIGHT + 1,
        0xFFFFFFFF,
    );

    buffer
}

/// Create a debug info overlay for N64
#[allow(clippy::too_many_arguments)]
pub fn create_n64_debug_overlay(
    width: usize,
    height: usize,
    rom_name: &str,
    rom_size_mb: f32,
    pc: u64,
    rsp_microcode: &str,
    rsp_vertex_count: usize,
    rdp_status: u32,
    framebuffer_resolution: &str,
    fps: f64,
    video_backend: &str,
) -> Vec<u32> {
    // Semi-transparent dark background
    let mut buffer = vec![0xC0000000; width * height];

    let rom_line = format!("ROM: {}", rom_name);
    let size_line = format!("Size: {:.1} MB", rom_size_mb);
    let fps_line = format!("FPS: {:.1}", fps);
    let video_line = format!("Video: {}", video_backend);

    let pc_line = format!("PC: 0x{:016X}", pc);
    let rsp_line = format!("RSP: {} microcode", rsp_microcode);
    let vtx_line = format!("VTX: {} vertices loaded", rsp_vertex_count);
    let rdp_line = format!("RDP: status=0x{:08X}", rdp_status);
    let fb_line = format!("FB: {}", framebuffer_resolution);

    let debug_lines: Vec<&str> = vec![
        "DEBUG INFO - N64",
        "",
        &rom_line,
        &size_line,
        &fps_line,
        &video_line,
        "",
        &pc_line,
        &rsp_line,
        &vtx_line,
        &rdp_line,
        &fb_line,
        "",
        "Press F10 to close",
    ];

    let start_x = 10;
    let start_y = 10;

    draw_text_lines(
        &mut buffer,
        width,
        height,
        &debug_lines,
        start_x,
        start_y,
        FONT_HEIGHT + 1,
        0xFFFFFFFF,
    );

    buffer
}

/// Create a debug info overlay for Atari 2600
#[allow(clippy::too_many_arguments)]
pub fn create_atari2600_debug_overlay(
    width: usize,
    height: usize,
    rom_size: usize,
    banking_scheme: &str,
    current_bank: usize,
    scanline: u64,
    fps: f64,
    video_backend: &str,
) -> Vec<u32> {
    let mut buffer = vec![0xC0000000; width * height];

    let size_line = format!("ROM Size: {} bytes", rom_size);
    let bank_line = format!("Banking: {} (cur {})", banking_scheme, current_bank);
    let scan_line = format!("Scanline: {}", scanline);
    let fps_line = format!("FPS: {:.1}", fps);
    let video_line = format!("Video: {}", video_backend);

    let debug_lines: Vec<&str> = vec![
        "DEBUG INFO - Atari 2600",
        "",
        &size_line,
        &bank_line,
        &scan_line,
        &fps_line,
        &video_line,
        "",
        "Press F10 to close",
    ];

    let start_x = 10;
    let start_y = 10;

    draw_text_lines(
        &mut buffer,
        width,
        height,
        &debug_lines,
        start_x,
        start_y,
        FONT_HEIGHT + 1,
        0xFFFFFFFF,
    );

    buffer
}

/// Create a debug info overlay for SNES
#[allow(clippy::too_many_arguments)]
pub fn create_snes_debug_overlay(
    width: usize,
    height: usize,
    rom_size: usize,
    has_smc_header: bool,
    pc: u16,
    pbr: u8,
    emulation_mode: bool,
    fps: f64,
    video_backend: &str,
) -> Vec<u32> {
    let mut buffer = vec![0xC0000000; width * height];

    let rom_kb = rom_size / 1024;
    let size_line = format!("ROM: {} KB", rom_kb);
    let header_line = if has_smc_header {
        "Header: SMC (512 bytes)"
    } else {
        "Header: None"
    };
    let fps_line = format!("FPS: {:.1}", fps);
    let video_line = format!("Video: {}", video_backend);

    let pc_line = format!("PC: 0x{:02X}:{:04X}", pbr, pc);
    let mode_line = if emulation_mode {
        "Mode: Emulation (6502)"
    } else {
        "Mode: Native (65C816)"
    };

    let debug_lines: Vec<&str> = vec![
        "DEBUG INFO - SNES",
        "",
        &size_line,
        header_line,
        &fps_line,
        &video_line,
        "",
        &pc_line,
        mode_line,
        "",
        "WARNING: Minimal PPU",
        "Most games will not display correctly",
        "",
        "Press F10 to close",
    ];

    let start_x = 10;
    let start_y = 10;

    draw_text_lines(
        &mut buffer,
        width,
        height,
        &debug_lines,
        start_x,
        start_y,
        FONT_HEIGHT + 1,
        0xFFFFFFFF,
    );

    buffer
}

/// Create a debug info overlay for PC
#[allow(clippy::too_many_arguments)]
pub fn create_pc_debug_overlay(
    width: usize,
    height: usize,
    cs: u16,
    ip: u16,
    ax: u16,
    bx: u16,
    cx: u16,
    dx: u16,
    sp: u16,
    bp: u16,
    si: u16,
    di: u16,
    flags: u16,
    cycles: u64,
    fps: f64,
    video_backend: &str,
) -> Vec<u32> {
    // Semi-transparent dark background
    let mut buffer = vec![0xC0000000; width * height];

    let cs_ip_line = format!("CS:IP: {:04X}:{:04X}", cs, ip);
    let ax_bx_line = format!("AX: {:04X}  BX: {:04X}", ax, bx);
    let cx_dx_line = format!("CX: {:04X}  DX: {:04X}", cx, dx);
    let sp_bp_line = format!("SP: {:04X}  BP: {:04X}", sp, bp);
    let si_di_line = format!("SI: {:04X}  DI: {:04X}", si, di);
    let flags_line = format!("FLAGS: {:04X}", flags);
    let cycles_line = format!("Cycles: {}", cycles);
    let fps_line = format!("FPS: {:.1}", fps);
    let video_line = format!("Video: {}", video_backend);

    // Extract flag bits
    let cf = if flags & 0x0001 != 0 { "C" } else { "-" };
    let pf = if flags & 0x0004 != 0 { "P" } else { "-" };
    let af = if flags & 0x0010 != 0 { "A" } else { "-" };
    let zf = if flags & 0x0040 != 0 { "Z" } else { "-" };
    let sf = if flags & 0x0080 != 0 { "S" } else { "-" };
    let tf = if flags & 0x0100 != 0 { "T" } else { "-" };
    let if_ = if flags & 0x0200 != 0 { "I" } else { "-" };
    let df = if flags & 0x0400 != 0 { "D" } else { "-" };
    let of = if flags & 0x0800 != 0 { "O" } else { "-" };
    let flags_decoded = format!(
        "Flags: {} {} {} {} {} {} {} {} {}",
        cf, pf, af, zf, sf, tf, if_, df, of
    );

    let debug_lines: Vec<&str> = vec![
        "DEBUG INFO - PC",
        "",
        &cs_ip_line,
        &ax_bx_line,
        &cx_dx_line,
        &sp_bp_line,
        &si_di_line,
        &flags_line,
        &flags_decoded,
        "",
        &cycles_line,
        &fps_line,
        &video_line,
        "",
        "Press F10 to close",
    ];

    let start_x = 10;
    let start_y = 10;

    draw_text_lines(
        &mut buffer,
        width,
        height,
        &debug_lines,
        start_x,
        start_y,
        FONT_HEIGHT + 1,
        0xFFFFFFFF,
    );

    buffer
}

/// Create a mount point selection overlay
pub fn create_mount_point_selector(
    width: usize,
    height: usize,
    mount_points: &[emu_core::MountPointInfo],
) -> Vec<u32> {
    // Semi-transparent dark background
    let mut buffer = vec![0xC0000000; width * height];

    let title = "SELECT MOUNT POINT";

    // We need to store the strings so they live long enough
    let lines_storage: Vec<String> = mount_points
        .iter()
        .enumerate()
        .map(|(i, mp)| format!("  {} - {}", i + 1, mp.name))
        .collect();

    let mut display_lines: Vec<&str> = vec![title, ""];
    for line in &lines_storage {
        display_lines.push(line.as_str());
    }
    display_lines.push("");
    display_lines.push("Press number to select, ESC to cancel");

    let start_x = (width.saturating_sub(30 * FONT_WIDTH)) / 2;
    let start_y = height / 3;

    draw_text_lines(
        &mut buffer,
        width,
        height,
        &display_lines,
        start_x,
        start_y,
        FONT_HEIGHT + 1,
        0xFFFFFFFF,
    );

    buffer
}

/// Create a speed selector overlay
pub fn create_speed_selector_overlay(width: usize, height: usize, current_speed: f64) -> Vec<u32> {
    // Semi-transparent dark background
    let mut buffer = vec![0xC0000000; width * height];

    let title = "EMULATION SPEED - Select (0-5)";

    // Speed options
    let speeds = [
        (0.0, "0 - Pause (0x)"),
        (0.25, "1 - Slow Motion (0.25x)"),
        (0.5, "2 - Half Speed (0.5x)"),
        (1.0, "3 - Normal (1x)"),
        (2.0, "4 - Fast Forward (2x)"),
        (10.0, "5 - Turbo (10x)"),
    ];

    let mut all_lines = Vec::new();
    all_lines.push(title);
    all_lines.push("");

    // Store the strings so they live long enough
    let mut speed_lines = Vec::new();
    for (speed_value, label) in &speeds {
        let marker = if (*speed_value - current_speed).abs() < 0.01 {
            ">"
        } else {
            " "
        };
        speed_lines.push(format!("{} {}", marker, label));
    }

    for line in &speed_lines {
        all_lines.push(line.as_str());
    }

    all_lines.push("");
    all_lines.push("Press 0-5 to select, ESC to cancel");

    let start_x = (width.saturating_sub(35 * FONT_WIDTH)) / 2;
    let start_y = height / 3;

    draw_text_lines(
        &mut buffer,
        width,
        height,
        &all_lines,
        start_x,
        start_y,
        FONT_HEIGHT + 2,
        0xFFFFFFFF,
    );

    buffer
}

/// Create splash screen with status message
pub fn create_splash_screen_with_status(
    width: usize,
    height: usize,
    status_message: &str,
) -> Vec<u32> {
    let mut buffer = vec![0xFF1A1A2E; width * height]; // Dark blue background

    // Hemulator logo/title - draw each line centered individually
    let logo_y = height / 3;

    // Center "HEMULATOR" (9 characters)
    let hemulator_x = (width - 9 * FONT_WIDTH) / 2;
    draw_text(
        &mut buffer,
        width,
        height,
        "HEMULATOR",
        hemulator_x,
        logo_y,
        0xFF16F2B3,
    );

    // Center "Multi-System Emulator" (21 characters)
    let subtitle_x = (width - 21 * FONT_WIDTH) / 2;
    draw_text(
        &mut buffer,
        width,
        height,
        "Multi-System Emulator",
        subtitle_x,
        logo_y + (FONT_HEIGHT + 4) * 2,
        0xFF16F2B3,
    );

    // Instructions - center each line individually
    let inst_y = height * 2 / 3;

    // "Press F3 to open a ROM" (22 characters)
    let inst1_x = (width - 22 * FONT_WIDTH) / 2;
    draw_text(
        &mut buffer,
        width,
        height,
        "Press F3 to open a ROM",
        inst1_x,
        inst_y,
        0xFFF0F0F0,
    );

    // "Press F7 to load project" (24 characters)
    let inst2_x = (width - 24 * FONT_WIDTH) / 2;
    draw_text(
        &mut buffer,
        width,
        height,
        "Press F7 to load project",
        inst2_x,
        inst_y + FONT_HEIGHT + 4,
        0xFFF0F0F0,
    );

    // "Press F1 for help" (17 characters)
    let inst3_x = (width - 17 * FONT_WIDTH) / 2;
    draw_text(
        &mut buffer,
        width,
        height,
        "Press F1 for help",
        inst3_x,
        inst_y + (FONT_HEIGHT + 4) * 2,
        0xFFF0F0F0,
    );

    // Status message at bottom - centered
    if !status_message.is_empty() {
        let status_y = height.saturating_sub(FONT_HEIGHT + 10);
        let status_x = (width.saturating_sub(status_message.len() * FONT_WIDTH)) / 2;
        draw_text(
            &mut buffer,
            width,
            height,
            status_message,
            status_x,
            status_y,
            0xFF16F2B3,
        );
    }

    buffer
}

/// Create system selector overlay
pub fn create_system_selector_overlay(width: usize, height: usize) -> Vec<u32> {
    // Semi-transparent dark background
    let mut buffer = vec![0xC0000000; width * height];

    let all_lines = vec![
        "SYSTEM SELECTOR",
        "",
        "  1 - NES (Nintendo Entertainment System)",
        "  2 - Game Boy",
        "  3 - Atari 2600",
        "  4 - PC (IBM PC/XT)",
        "  5 - SNES (Super Nintendo)",
        "  6 - N64 (Nintendo 64)",
        "",
        "Press 1-6 to select, ESC to cancel",
    ];

    let start_x = (width.saturating_sub(45 * FONT_WIDTH)) / 2;
    let start_y = height / 3;

    draw_text_lines(
        &mut buffer,
        width,
        height,
        &all_lines,
        start_x,
        start_y,
        FONT_HEIGHT + 2,
        0xFFFFFFFF,
    );

    buffer
}
