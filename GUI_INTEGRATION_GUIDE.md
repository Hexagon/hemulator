# GUI Integration Guide

This document explains how to integrate the new egui-based tabbed GUI with menu system into the emulator.

## Overview

The new GUI system adds:
- **Menu bar** with File, Machine, Display, Devices, and Help menus
- **Tab system** with Monitor, Debug, and Log Output tabs
- **Status bar** showing system info, FPS, and status messages
- **Cross-platform** support (Windows, Linux, macOS)
- **Both OpenGL and software rendering** modes

## Architecture

### Current State
The emulator currently uses:
- SDL2 for window management and input
- Custom overlays rendered via `ui_render.rs` for help, debug info, etc.
- F-key shortcuts for all operations

### New Components

1. **egui_gui.rs** - Egui GUI state and rendering logic
   - `EguiGui` struct manages GUI state
   - `GuiAction` enum for menu/UI actions
   - `ActiveTab` enum for tab selection
   - `render_basic()` method to render the GUI

2. **Integration Requirements** (To be implemented)
   - Add egui_glow integration to SDL2 backend
   - Create egui context and painter
   - Handle input events for egui (mouse, keyboard)
   - Render egui UI on top of emulator framebuffer
   - Process `GuiAction` results in main loop

## Integration Steps

### Phase 1: Basic egui Setup (CURRENT)
- [x] Add egui and egui_glow dependencies
- [x] Create egui_gui module with basic menu/tab structure
- [x] Define GuiAction enum for menu actions
- [ ] Test basic build

### Phase 2: SDL2 + egui Integration
This phase requires modifying the window backend to support egui rendering.

#### Option A: Modify Sdl2Backend directly
```rust
// In sdl2_backend.rs
pub struct Sdl2Backend {
    // ... existing fields ...
    egui_ctx: Option<egui::Context>,
    egui_painter: Option<egui_glow::Painter>,
    egui_input: egui::RawInput,
}
```

#### Option B: Create wrapper (Recommended for minimal changes)
```rust
// New file: egui_sdl2_integration.rs
pub struct EguiSdl2Integration {
    egui_ctx: egui::Context,
    egui_painter: egui_glow::Painter,
    start_time: std::time::Instant,
}

impl EguiSdl2Integration {
    pub fn new(gl_context: &glow::Context, window: &sdl2::video::Window) -> Self { ... }
    pub fn handle_event(&mut self, event: &sdl2::event::Event) { ... }
    pub fn begin_frame(&mut self, window: &sdl2::video::Window) -> egui::Context { ... }
    pub fn end_frame(&mut self, gl_context: &glow::Context, output: egui::FullOutput) { ... }
}
```

### Phase 3: Main Loop Integration
Modify main.rs event loop:

```rust
// In main.rs main() function
let mut egui_gui = EguiGui::new();
let mut egui_integration = EguiSdl2Integration::new(...);

// In main event loop, before rendering:
window.poll_events(); // existing
egui_integration.handle_events(&window.events()); // new

// Render emulator frame as usual
// ...

// Then render egui on top:
let egui_ctx = egui_integration.begin_frame(&window);
let action = egui_gui.render_basic(&egui_ctx);
let output = egui_ctx.end_frame();
egui_integration.end_frame(&gl_context, output);

// Process the action:
match action {
    GuiAction::Exit => break,
    GuiAction::SelectCrtFilter(filter) => {
        settings.display_filter = filter;
        // ... apply filter
    }
    // ... handle other actions
    _ => {}
}
```

### Phase 4: F-Key Compatibility
Keep F-key shortcuts working alongside menu system:
- F1 → Help (could toggle help tab)
- F3 → Open mount point (same as Devices menu)
- F11 → Cycle CRT filter (same as Display > Filters)
- etc.

### Phase 5: Tab Content
Implement tab-specific rendering:
- **Monitor tab**: Show emulator framebuffer (default)
- **Debug tab**: Show debug info panel (currently overlay)
- **Log Output tab**: Capture and display log messages with filters

## Implementation Challenges

### 1. OpenGL Context Sharing
- Emulator uses OpenGL for rendering framebuffer
- egui_glow also needs OpenGL context
- Solution: Use same GL context, render emulator first, then egui

### 2. Input Routing
- egui needs mouse/keyboard input for menus
- Emulator needs keyboard input for controls
- Solution: 
  - Let egui consume input first
  - Only pass through to emulator if egui didn't want it
  - Use `ctx.wants_pointer_input()` and `ctx.wants_keyboard_input()`

### 3. Software Rendering Mode
- egui_glow requires OpenGL
- Current software mode uses SDL2 canvas
- Solutions:
  - Option A: Require OpenGL for GUI mode
  - Option B: Use egui-sdl2 crate (different integration)
  - Option C: Fallback to F-key mode when software rendering

### 4. Performance
- egui rendering adds overhead
- Solution: Only render GUI when visible/changed
- Use `ctx.request_repaint_after()` for efficient updates

## Testing Plan

1. **Basic Integration Test**
   - Create minimal example showing egui menu on black screen
   - Verify menus work, tabs switch

2. **Emulator Integration Test**
   - Add egui to actual emulator
   - Verify emulator still runs at full speed
   - Verify input routing works correctly

3. **Cross-Platform Test**
   - Test on Linux (primary platform)
   - Test on Windows (if available)
   - Test on macOS (if available)

4. **Functionality Test**
   - All menu items trigger correct actions
   - Tabs switch properly
   - Status bar updates correctly
   - F-keys still work as shortcuts

## Migration Path

For users of existing F-key system:
1. New GUI is opt-in initially (toggle with F1 or command-line flag)
2. All F-keys continue to work
3. Document new menu system in MANUAL.md
4. Eventually make GUI default, with option to disable

## Files to Modify

### Phase 2-3 (Integration)
- `crates/frontend/gui/src/window_backend/sdl2_backend.rs` - Add egui support
- `crates/frontend/gui/src/main.rs` - Integrate egui into main loop

### Phase 4 (Tab Content)
- `crates/frontend/gui/src/egui_gui.rs` - Enhanced tab rendering
- `crates/frontend/gui/src/main.rs` - Pass system state to GUI

### Documentation
- `MANUAL.md` - Document new GUI and menus
- `README.md` - Update screenshots/description
- `AGENTS.md` - Update with GUI info if needed

## Current Status

✅ **Completed:**
- Basic egui module structure
- GuiAction enum for menu actions
- Tab system framework
- Dependencies added

⏳ **In Progress:**
- Integration guide (this document)

📋 **TODO:**
- SDL2 + egui integration layer
- Main loop modifications
- Input routing
- Tab content implementation
- Testing
- Documentation updates

## References

- [egui documentation](https://docs.rs/egui/)
- [egui_glow documentation](https://docs.rs/egui_glow/)
- [egui examples](https://github.com/emilk/egui/tree/master/examples)
