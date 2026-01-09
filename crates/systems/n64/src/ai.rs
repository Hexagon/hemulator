//! AI (Audio Interface) - Audio output controller for Nintendo 64
//!
//! The AI is responsible for:
//! - Managing audio DMA from RDRAM to the DAC
//! - Controlling audio sample rate and bit depth
//! - Generating audio interrupts
//!
//! ## Memory Map
//!
//! AI registers are memory-mapped at 0x04500000-0x04500017:
//! - 0x04500000: AI_DRAM_ADDR - DMA source address in RDRAM
//! - 0x04500004: AI_LEN - Transfer length (bytes to transfer)
//! - 0x04500008: AI_CONTROL - Audio control (DMA enable, frequency)
//! - 0x0450000C: AI_STATUS - Audio status (DMA busy, full)
//! - 0x04500010: AI_DACRATE - DAC sample rate
//! - 0x04500014: AI_BITRATE - Bit rate control
//!
//! ## Audio DMA
//!
//! The AI uses DMA to transfer audio samples from RDRAM to the DAC:
//! 1. CPU writes RDRAM address to AI_DRAM_ADDR
//! 2. CPU writes transfer length to AI_LEN (triggers DMA)
//! 3. AI transfers samples from RDRAM to audio buffer
//! 4. AI generates interrupt when transfer completes
//!
//! ## Sample Format
//!
//! N64 audio is typically:
//! - 16-bit signed PCM samples
//! - Stereo (left/right interleaved)
//! - Sample rates: 22050 Hz, 32000 Hz, 44100 Hz, 48000 Hz
//! - Big-endian format (MIPS byte order)

use emu_core::logging::{log, LogCategory, LogLevel};

/// AI register offsets (relative to 0x04500000)
const AI_DRAM_ADDR: u32 = 0x00;
const AI_LEN: u32 = 0x04;
const AI_CONTROL: u32 = 0x08;
const AI_STATUS: u32 = 0x0C;
const AI_DACRATE: u32 = 0x10;
const AI_BITRATE: u32 = 0x14;

/// AI_STATUS register bits
const AI_STATUS_DMA_BUSY: u32 = 0x40000000; // DMA transfer in progress
const AI_STATUS_FULL: u32 = 0x80000000; // Audio buffer full

/// Audio Interface controller
pub struct AudioInterface {
    /// DMA source address in RDRAM
    dram_addr: u32,

    /// Transfer length in bytes
    len: u32,

    /// Audio control register
    control: u32,

    /// Audio status flags
    status: u32,

    /// DAC sample rate (CPU cycles per sample)
    dacrate: u32,

    /// Bit rate control
    bitrate: u32,

    /// Audio buffer for samples (stereo 16-bit PCM)
    /// Max buffer size: 128KB for smooth playback
    audio_buffer: Vec<i16>,

    /// Current playback position in buffer
    playback_position: usize,

    /// DMA completion pending (triggers AI interrupt)
    dma_complete_pending: bool,
}

impl AudioInterface {
    /// Create a new Audio Interface with default settings
    pub fn new() -> Self {
        Self {
            dram_addr: 0,
            len: 0,
            control: 0,
            status: 0,
            dacrate: 0x0000_0C00, // Default ~48 kHz (CPU freq / 48000)
            bitrate: 0,
            audio_buffer: Vec::with_capacity(65536), // 64K samples = 128KB
            playback_position: 0,
            dma_complete_pending: false,
        }
    }

    /// Reset to initial state
    #[allow(dead_code)] // Public API for future audio reset
    pub fn reset(&mut self) {
        self.dram_addr = 0;
        self.len = 0;
        self.control = 0;
        self.status = 0;
        self.dacrate = 0x0000_0C00;
        self.bitrate = 0;
        self.audio_buffer.clear();
        self.playback_position = 0;
        self.dma_complete_pending = false;
    }

    /// Read from AI register
    pub fn read_register(&self, offset: u32) -> u32 {
        match offset {
            AI_DRAM_ADDR => self.dram_addr,
            AI_LEN => self.len,
            AI_CONTROL => self.control,
            AI_STATUS => self.status,
            AI_DACRATE => self.dacrate,
            AI_BITRATE => self.bitrate,
            _ => {
                log(LogCategory::Stubs, LogLevel::Warn, || {
                    format!("N64 AI: Read from unknown register offset 0x{:02X}", offset)
                });
                0
            }
        }
    }

    /// Write to AI register
    pub fn write_register(&mut self, offset: u32, value: u32, rdram: &[u8]) {
        match offset {
            AI_DRAM_ADDR => {
                self.dram_addr = value & 0x00FFFFFF; // 24-bit address
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!("N64 AI: Set DRAM address to 0x{:08X}", self.dram_addr)
                });
            }
            AI_LEN => {
                self.len = value & 0x0003FFFF; // 18-bit length (max 256KB)
                log(LogCategory::PPU, LogLevel::Info, || {
                    format!(
                        "N64 AI: DMA transfer - addr=0x{:08X}, len={} bytes",
                        self.dram_addr, self.len
                    )
                });

                // Trigger DMA transfer when length is written
                self.transfer_audio_dma(rdram);
            }
            AI_CONTROL => {
                self.control = value & 0x01; // Only bit 0 is used (DMA enable)
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!("N64 AI: Control = 0x{:08X}", self.control)
                });
            }
            AI_STATUS => {
                // Status register is read-only, but writing clears interrupt
                self.dma_complete_pending = false;
                log(LogCategory::PPU, LogLevel::Debug, || {
                    "N64 AI: Status write - clearing interrupt".to_string()
                });
            }
            AI_DACRATE => {
                self.dacrate = value & 0x00003FFF; // 14-bit DAC rate
                let sample_rate = self.calculate_sample_rate();
                log(LogCategory::PPU, LogLevel::Info, || {
                    format!(
                        "N64 AI: DAC rate set to 0x{:04X} (~{} Hz)",
                        self.dacrate, sample_rate
                    )
                });
            }
            AI_BITRATE => {
                self.bitrate = value & 0x0000000F; // 4-bit bitrate
                log(LogCategory::PPU, LogLevel::Debug, || {
                    format!("N64 AI: Bit rate = 0x{:X}", self.bitrate)
                });
            }
            _ => {
                log(LogCategory::Stubs, LogLevel::Warn, || {
                    format!(
                        "N64 AI: Write to unknown register offset 0x{:02X} = 0x{:08X}",
                        offset, value
                    )
                });
            }
        }
    }

    /// Transfer audio samples from RDRAM via DMA
    fn transfer_audio_dma(&mut self, rdram: &[u8]) {
        if self.len == 0 {
            return;
        }

        // Set busy flag
        self.status |= AI_STATUS_DMA_BUSY;

        let addr = (self.dram_addr & 0x00FFFFFF) as usize;
        let len = self.len as usize;

        // Audio samples are 16-bit stereo (4 bytes per sample pair)
        // Transfer samples from RDRAM to audio buffer
        if addr + len <= rdram.len() {
            // Convert bytes to 16-bit samples (big-endian)
            let num_samples = len / 2; // 2 bytes per 16-bit sample

            // Prevent buffer overflow - max 256K samples (512KB)
            const MAX_BUFFER_SAMPLES: usize = 256 * 1024;
            let available_space = MAX_BUFFER_SAMPLES.saturating_sub(self.audio_buffer.len());

            if available_space == 0 {
                // Buffer is full - set full flag and skip transfer
                self.status |= AI_STATUS_FULL;
                self.status &= !AI_STATUS_DMA_BUSY;
                log(LogCategory::PPU, LogLevel::Warn, || {
                    "N64 AI: Audio buffer full, dropping samples".to_string()
                });
                return;
            }

            let samples_to_transfer = num_samples.min(available_space);

            for i in 0..samples_to_transfer {
                let byte_offset = addr + (i * 2);
                if byte_offset + 1 < rdram.len() {
                    let sample = i16::from_be_bytes([rdram[byte_offset], rdram[byte_offset + 1]]);
                    self.audio_buffer.push(sample);
                }
            }

            // Set full flag if buffer is near capacity
            if self.audio_buffer.len() >= MAX_BUFFER_SAMPLES - 1024 {
                self.status |= AI_STATUS_FULL;
            }

            log(LogCategory::PPU, LogLevel::Debug, || {
                format!(
                    "N64 AI: Transferred {} samples ({} bytes) from RDRAM 0x{:08X}",
                    samples_to_transfer, len, addr
                )
            });
        } else {
            log(LogCategory::PPU, LogLevel::Warn, || {
                format!(
                    "N64 AI: DMA address out of bounds: addr=0x{:08X}, len={}",
                    addr, len
                )
            });
        }

        // Clear busy flag and set completion
        self.status &= !AI_STATUS_DMA_BUSY;
        self.dma_complete_pending = true;
    }

    /// Calculate sample rate from DAC rate register
    /// CPU frequency is ~93.75 MHz, DAC rate divides this
    fn calculate_sample_rate(&self) -> u32 {
        if self.dacrate == 0 {
            return 44100; // Default
        }

        // N64 CPU frequency: 93750000 Hz
        // Sample rate = CPU freq / (dacrate + 1)
        const CPU_FREQ: u32 = 93750000;
        CPU_FREQ / (self.dacrate + 1)
    }

    /// Get audio samples for playback
    /// Returns up to `count` stereo samples and removes them from the buffer
    #[allow(dead_code)] // Public API for future audio output
    pub fn get_audio_samples(&mut self, count: usize) -> Vec<i16> {
        let available = self.audio_buffer.len();
        let to_take = count.min(available);

        // Take samples from the front of the buffer
        let samples: Vec<i16> = self.audio_buffer.drain(..to_take).collect();

        // Update buffer status
        if self.audio_buffer.len() < 1024 {
            self.status &= !AI_STATUS_FULL;
        }

        samples
    }

    /// Check if AI interrupt is pending
    pub fn is_interrupt_pending(&self) -> bool {
        self.dma_complete_pending
    }

    /// Clear AI interrupt
    #[allow(dead_code)] // Public API for interrupt handling
    pub fn clear_interrupt(&mut self) {
        self.dma_complete_pending = false;
    }

    /// Get current sample rate
    #[allow(dead_code)] // Public API for audio configuration
    pub fn get_sample_rate(&self) -> u32 {
        self.calculate_sample_rate()
    }

    /// Get number of buffered samples
    #[allow(dead_code)] // Public API for buffer monitoring
    pub fn buffered_samples(&self) -> usize {
        self.audio_buffer.len()
    }
}

impl Default for AudioInterface {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_creation() {
        let ai = AudioInterface::new();
        assert_eq!(ai.dram_addr, 0);
        assert_eq!(ai.len, 0);
        assert_eq!(ai.status, 0);
    }

    #[test]
    fn test_ai_reset() {
        let mut ai = AudioInterface::new();
        ai.dram_addr = 0x1000;
        ai.len = 0x400;
        ai.status = AI_STATUS_DMA_BUSY;

        ai.reset();

        assert_eq!(ai.dram_addr, 0);
        assert_eq!(ai.len, 0);
        assert_eq!(ai.status, 0);
    }

    #[test]
    fn test_sample_rate_calculation() {
        let mut ai = AudioInterface::new();

        // Test common sample rates
        // 48000 Hz: 93750000 / 48000 = 1953.125 ≈ 1953
        ai.dacrate = 1952; // (dacrate + 1) = 1953
        let rate = ai.calculate_sample_rate();
        assert!((47900..=48100).contains(&rate)); // Allow small rounding error

        // 44100 Hz: 93750000 / 44100 = 2125.85 ≈ 2126
        ai.dacrate = 2125; // (dacrate + 1) = 2126
        let rate = ai.calculate_sample_rate();
        assert!((44000..=44200).contains(&rate)); // Allow small rounding error
    }

    #[test]
    fn test_audio_dma_transfer() {
        let mut ai = AudioInterface::new();
        let mut rdram = vec![0u8; 0x400000]; // 4MB

        // Write test samples to RDRAM (16-bit big-endian)
        rdram[0x1000] = 0x12;
        rdram[0x1001] = 0x34;
        rdram[0x1002] = 0x56;
        rdram[0x1003] = 0x78;

        // Set up DMA transfer
        ai.dram_addr = 0x1000;
        ai.len = 4; // 4 bytes = 2 samples
        ai.transfer_audio_dma(&rdram);

        // Check samples were transferred
        assert_eq!(ai.audio_buffer.len(), 2);
        assert_eq!(ai.audio_buffer[0], 0x1234i16);
        assert_eq!(ai.audio_buffer[1], 0x5678i16);
        assert!(ai.is_interrupt_pending());
    }

    #[test]
    fn test_get_audio_samples() {
        let mut ai = AudioInterface::new();
        ai.audio_buffer = vec![100, 200, 300, 400, 500];

        let samples = ai.get_audio_samples(3);

        assert_eq!(samples.len(), 3);
        assert_eq!(samples, vec![100, 200, 300]);
        assert_eq!(ai.audio_buffer.len(), 2);
        assert_eq!(ai.audio_buffer, vec![400, 500]);
    }
}
