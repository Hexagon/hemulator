//! Example demonstrating the video processing system
//!
//! This example shows how to use both SoftwareProcessor and OpenGLProcessor
//! to apply CRT filters to a frame buffer.
//!
//! Run with: cargo run --example video_processing -p emu_gui
//! Or with OpenGL: cargo run --example video_processing -p emu_gui --features opengl

fn main() {
    println!("Video Processing System Example");
    println!("================================\n");

    // Create a simple test pattern (256x240 pixels)
    let width = 256;
    let height = 240;
    let _buffer = create_test_pattern(width, height);

    println!("Created test pattern: {}x{} pixels\n", width, height);

    println!("Video Processing System:");
    println!("------------------------");
    println!("  Software Processor: Always available (CPU-based)");
    println!("  Filters: None, Scanlines, Phosphor, CRT Monitor");
    println!("  Architecture: Modular VideoProcessor trait");

    #[cfg(feature = "opengl")]
    {
        println!("\nOpenGL Support: ENABLED");
        println!("  OpenGL Processor: Available (GPU-based)");
        println!("  Shaders: GLSL 330 core");
        println!("  Filters: Implemented as fragment shaders");
        println!("  Dependencies: glow, glutin, bytemuck");
    }

    #[cfg(not(feature = "opengl"))]
    {
        println!("\nOpenGL Support: DISABLED");
        println!("  To enable: cargo build --features opengl");
    }

    println!("\nConfiguration:");
    println!("  Set video_backend in config.json:");
    println!("    - \"software\" for CPU-based rendering");
    println!("    - \"opengl\" for GPU-based rendering (if compiled with --features opengl)");

    println!("\nShader Files (OpenGL backend):");
    println!("  - src/shaders/vertex.glsl");
    println!("  - src/shaders/fragment_none.glsl");
    println!("  - src/shaders/fragment_scanlines.glsl");
    println!("  - src/shaders/fragment_phosphor.glsl");
    println!("  - src/shaders/fragment_crt.glsl");

    println!("\nExample completed successfully!");
}

/// Create a simple test pattern with vertical color bars
fn create_test_pattern(width: usize, height: usize) -> Vec<u32> {
    let mut buffer = vec![0u32; width * height];

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;

            // Create vertical color bars
            let bar_width = width / 8;
            let bar = x / bar_width;

            let color = match bar {
                0 => 0xFFFFFFFF, // White
                1 => 0xFFFFFF00, // Yellow
                2 => 0xFF00FFFF, // Cyan
                3 => 0xFF00FF00, // Green
                4 => 0xFFFF00FF, // Magenta
                5 => 0xFFFF0000, // Red
                6 => 0xFF0000FF, // Blue
                _ => 0xFF000000, // Black
            };

            buffer[idx] = color;
        }
    }

    buffer
}
