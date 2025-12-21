//! SDL2 window backend supporting both software and OpenGL rendering

use super::{Key, WindowBackend};
use crate::crt_filter::CrtFilter;
use crate::video_processor::{OpenGLProcessor, SoftwareProcessor, VideoProcessor};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::Canvas;
use sdl2::video::{GLProfile, Window};
use sdl2::{EventPump, Sdl, VideoSubsystem};
use std::collections::HashSet;
use std::error::Error;

pub enum RenderMode {
    Software {
        canvas: Canvas<Window>,
        processor: SoftwareProcessor,
    },
    OpenGL {
        window: Window,
        _gl_context: sdl2::video::GLContext,
        processor: Box<OpenGLProcessor>,
    },
}

pub struct Sdl2Backend {
    _sdl_context: Sdl,
    _video_subsystem: VideoSubsystem,
    render_mode: RenderMode,
    event_pump: EventPump,
    pressed_keys: HashSet<Key>,
    key_pressed_once: HashSet<Key>,
    is_open: bool,
    current_filter: CrtFilter,
}

impl Sdl2Backend {
    pub fn new(
        title: &str,
        width: usize,
        height: usize,
        use_opengl: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let sdl_context = sdl2::init()?;
        let video_subsystem = sdl_context.video()?;

        let render_mode = if use_opengl {
            // Configure OpenGL
            let gl_attr = video_subsystem.gl_attr();
            gl_attr.set_context_profile(GLProfile::Core);
            gl_attr.set_context_version(3, 3);

            let window = video_subsystem
                .window(title, width as u32, height as u32)
                .opengl()
                .resizable()
                .build()?;

            let gl_context = window.gl_create_context()?;
            window.gl_make_current(&gl_context)?;

            // Load GL functions
            let gl = unsafe {
                glow::Context::from_loader_function(|s| {
                    video_subsystem.gl_get_proc_address(s) as *const _
                })
            };

            let mut gl_processor = OpenGLProcessor::new(gl)?;
            gl_processor.init(width, height)?;

            RenderMode::OpenGL {
                window,
                _gl_context: gl_context,
                processor: Box::new(gl_processor),
            }
        } else {
            // Software rendering
            let window = video_subsystem
                .window(title, width as u32, height as u32)
                .resizable()
                .build()?;

            let canvas = window.into_canvas().build()?;
            let processor = SoftwareProcessor::new();

            RenderMode::Software { canvas, processor }
        };

        let event_pump = sdl_context.event_pump()?;

        Ok(Self {
            _sdl_context: sdl_context,
            _video_subsystem: video_subsystem,
            render_mode,
            event_pump,
            pressed_keys: HashSet::new(),
            key_pressed_once: HashSet::new(),
            is_open: true,
            current_filter: CrtFilter::None,
        })
    }

    /// Convert SDL2 Keycode to our unified Key
    fn from_sdl2_key(k: Keycode) -> Option<Key> {
        match k {
            Keycode::F1 => Some(Key::F1),
            Keycode::F2 => Some(Key::F2),
            Keycode::F3 => Some(Key::F3),
            Keycode::F4 => Some(Key::F4),
            Keycode::F5 => Some(Key::F5),
            Keycode::F6 => Some(Key::F6),
            Keycode::F7 => Some(Key::F7),
            Keycode::F8 => Some(Key::F8),
            Keycode::F9 => Some(Key::F9),
            Keycode::F10 => Some(Key::F10),
            Keycode::F11 => Some(Key::F11),
            Keycode::F12 => Some(Key::F12),
            Keycode::Num0 => Some(Key::Key0),
            Keycode::Num1 => Some(Key::Key1),
            Keycode::Num2 => Some(Key::Key2),
            Keycode::Num3 => Some(Key::Key3),
            Keycode::Num4 => Some(Key::Key4),
            Keycode::Num5 => Some(Key::Key5),
            Keycode::Num6 => Some(Key::Key6),
            Keycode::Num7 => Some(Key::Key7),
            Keycode::Num8 => Some(Key::Key8),
            Keycode::Num9 => Some(Key::Key9),
            Keycode::A => Some(Key::A),
            Keycode::B => Some(Key::B),
            Keycode::C => Some(Key::C),
            Keycode::D => Some(Key::D),
            Keycode::E => Some(Key::E),
            Keycode::F => Some(Key::F),
            Keycode::G => Some(Key::G),
            Keycode::H => Some(Key::H),
            Keycode::I => Some(Key::I),
            Keycode::J => Some(Key::J),
            Keycode::K => Some(Key::K),
            Keycode::L => Some(Key::L),
            Keycode::M => Some(Key::M),
            Keycode::N => Some(Key::N),
            Keycode::O => Some(Key::O),
            Keycode::P => Some(Key::P),
            Keycode::Q => Some(Key::Q),
            Keycode::R => Some(Key::R),
            Keycode::S => Some(Key::S),
            Keycode::T => Some(Key::T),
            Keycode::U => Some(Key::U),
            Keycode::V => Some(Key::V),
            Keycode::W => Some(Key::W),
            Keycode::X => Some(Key::X),
            Keycode::Y => Some(Key::Y),
            Keycode::Z => Some(Key::Z),
            Keycode::Up => Some(Key::Up),
            Keycode::Down => Some(Key::Down),
            Keycode::Left => Some(Key::Left),
            Keycode::Right => Some(Key::Right),
            Keycode::Escape => Some(Key::Escape),
            Keycode::Return => Some(Key::Enter),
            Keycode::Space => Some(Key::Space),
            Keycode::Tab => Some(Key::Tab),
            Keycode::Backspace => Some(Key::Backspace),
            Keycode::LShift => Some(Key::LeftShift),
            Keycode::RShift => Some(Key::RightShift),
            Keycode::LCtrl => Some(Key::LeftCtrl),
            Keycode::RCtrl => Some(Key::RightCtrl),
            Keycode::LAlt => Some(Key::LeftAlt),
            Keycode::RAlt => Some(Key::RightAlt),
            _ => None,
        }
    }

    pub fn set_filter(&mut self, filter: CrtFilter) {
        self.current_filter = filter;
    }
}

impl WindowBackend for Sdl2Backend {
    fn is_open(&self) -> bool {
        self.is_open
    }

    fn is_key_down(&self, key: Key) -> bool {
        self.pressed_keys.contains(&key)
    }

    fn is_key_pressed(&self, key: Key, _allow_repeat: bool) -> bool {
        self.key_pressed_once.contains(&key)
    }

    fn update_with_buffer(
        &mut self,
        buffer: &[u32],
        width: usize,
        height: usize,
    ) -> Result<(), Box<dyn Error>> {
        match &mut self.render_mode {
            RenderMode::OpenGL {
                window, processor, ..
            } => {
                // Render using OpenGL processor
                let _processed =
                    processor.process_frame(buffer, width, height, self.current_filter)?;

                // Swap buffers
                window.gl_swap_window();
            }
            RenderMode::Software { canvas, processor } => {
                // Process frame with software processor
                let processed =
                    processor.process_frame(buffer, width, height, self.current_filter)?;

                // Create texture for this frame
                let texture_creator = canvas.texture_creator();
                let mut texture = texture_creator.create_texture_streaming(
                    PixelFormatEnum::ARGB8888,
                    width as u32,
                    height as u32,
                )?;

                // Update texture with processed buffer
                texture.update(None, bytemuck::cast_slice(&processed), width * 4)?;

                // Clear and render
                canvas.clear();
                canvas.copy(&texture, None, None)?;
                canvas.present();
            }
        }

        Ok(())
    }

    fn get_size(&self) -> (usize, usize) {
        match &self.render_mode {
            RenderMode::OpenGL { window, .. } => {
                let size = window.size();
                (size.0 as usize, size.1 as usize)
            }
            RenderMode::Software { canvas, .. } => {
                let size = canvas.window().size();
                (size.0 as usize, size.1 as usize)
            }
        }
    }

    fn poll_events(&mut self) {
        // Clear one-time press flags at start of frame
        self.key_pressed_once.clear();

        // Poll all events
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    self.is_open = false;
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    repeat: false,
                    ..
                } => {
                    if let Some(key) = Self::from_sdl2_key(keycode) {
                        self.pressed_keys.insert(key);
                        self.key_pressed_once.insert(key);
                    }
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    repeat: true,
                    ..
                } => {
                    if let Some(key) = Self::from_sdl2_key(keycode) {
                        self.pressed_keys.insert(key);
                        // Don't add to key_pressed_once for repeat events
                    }
                }
                Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => {
                    if let Some(key) = Self::from_sdl2_key(keycode) {
                        self.pressed_keys.remove(&key);
                    }
                }
                _ => {}
            }
        }
    }

    fn name(&self) -> &str {
        match &self.render_mode {
            RenderMode::OpenGL { .. } => "SDL2 (OpenGL)",
            RenderMode::Software { .. } => "SDL2 (Software)",
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
