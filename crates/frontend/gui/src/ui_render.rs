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

    // Create strings that live long enough
    let a_line = format!("  {} - A button", settings.keyboard.a);
    let b_line = format!("  {} - B button", settings.keyboard.b);
    let select_line = format!("  {} - Select", settings.keyboard.select);
    let start_line = format!("  {} - Start", settings.keyboard.start);
    let dpad_line = format!(
        "  {} {} {} {} - D-pad",
        settings.keyboard.up,
        settings.keyboard.down,
        settings.keyboard.left,
        settings.keyboard.right
    );

    let help_lines: Vec<&str> = vec![
        "HEMULATOR - Help",
        "",
        "Controller:",
        &a_line,
        &b_line,
        &select_line,
        &start_line,
        &dpad_line,
        "",
        "Keys:",
        "  F1  - Help",
        "  F3  - Open ROM",
        "  F5  - Save state",
        "  F6  - Load state",
        "  F11 - Scale",
        "  F12 - Reset",
        "  ESC - Exit",
        "",
        "Press F1 to close",
    ];

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
        FONT_HEIGHT + 2,
        0xFFFFFFFF,
    );

    buffer
}
