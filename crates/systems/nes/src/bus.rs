use crate::apu::APU;
use crate::cartridge::Cartridge;
use crate::mappers::Mapper;
use crate::ppu::Ppu;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::OnceLock;

fn log_ppu_writes() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        matches!(
            std::env::var("EMU_LOG_PPU_WRITES").as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE")
        )
    })
}

pub trait Bus {
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
}

#[allow(dead_code)]
pub struct SimpleBus {
    ram: [u8; 0x800],
}

impl SimpleBus {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self { ram: [0; 0x800] }
    }
}

impl Bus for SimpleBus {
    fn read(&self, addr: u16) -> u8 {
        let a = addr as usize & 0x07FF;
        self.ram[a]
    }
    fn write(&mut self, addr: u16, val: u8) {
        let a = addr as usize & 0x07FF;
        self.ram[a] = val;
    }
}

#[derive(Debug)]
pub struct NesBus {
    pub ram: [u8; 0x800],
    pub wram: [u8; 0x2000],
    pub ppu: Ppu,
    pub apu: APU,
    mapper: Option<Rc<RefCell<Mapper>>>,
    // Simple controller state: each u8 is 8-button shift register (bit0 first)
    pub controller_state: [u8; 2],
    controller_shift: [Cell<u8>; 2],
    strobe: Cell<bool>,
}

impl NesBus {
    pub fn new(ppu: Ppu) -> Self {
        Self {
            ram: [0; 0x800],
            wram: [0; 0x2000],
            ppu,
            apu: APU::new(),
            mapper: None,
            controller_state: [0; 2],
            controller_shift: [Cell::new(0), Cell::new(0)],
            strobe: Cell::new(false),
        }
    }

    pub fn install_cart(&mut self, cart: Cartridge) {
        let mapper = Mapper::from_cart(cart, &mut self.ppu);
        let rc = Rc::new(RefCell::new(mapper));

        // Wire PPU A12 transitions to the mapper for IRQ clocking (e.g., MMC3).
        let weak: Weak<RefCell<Mapper>> = Rc::downgrade(&rc);
        self.ppu.set_a12_callback(Some(Box::new(move |a12_high| {
            if let Some(m) = weak.upgrade() {
                m.borrow_mut().notify_a12(a12_high);
            }
        })));

        self.mapper = Some(rc);
    }

    pub fn prg_rom(&self) -> Option<Vec<u8>> {
        self.mapper.as_ref().map(|m| m.borrow().prg_rom().to_vec())
    }

    pub fn take_irq_pending(&mut self) -> bool {
        if let Some(m) = &mut self.mapper {
            m.borrow_mut().take_irq_pending()
        } else {
            false
        }
    }

    /// Set controller state (8 buttons) for controller `idx` (0 or 1).
    pub fn set_controller(&mut self, idx: usize, state: u8) {
        if idx < 2 {
            self.controller_state[idx] = state;
        }
    }
}

impl Bus for NesBus {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => {
                // internal RAM mirrored
                let a = (addr as usize) & 0x07FF;
                self.ram[a]
            }
            0x2000..=0x3FFF => {
                // PPU registers mirrored every 8
                let reg = 0x2000 + (addr - 0x2000) % 8;
                // Forward reads to the PPU; `read_register` handles buffering/side effects.
                self.ppu.read_register(reg)
            }
            0x4000..=0x4017 => {
                // 0x4016: controller 1, 0x4017: controller 2
                match addr {
                    0x4016 => {
                        // When strobed, return current button A state (bit 0).
                        // When not strobed, shift out latched controller bits.
                        if self.strobe.get() {
                            self.controller_state[0] & 1
                        } else {
                            let cur = self.controller_shift[0].get();
                            let v = cur & 1;
                            self.controller_shift[0].set(cur >> 1);
                            v
                        }
                    }
                    0x4017 => {
                        if self.strobe.get() {
                            self.controller_state[1] & 1
                        } else {
                            let cur = self.controller_shift[1].get();
                            let v = cur & 1;
                            self.controller_shift[1].set(cur >> 1);
                            v
                        }
                    }
                    _ => 0,
                }
            }
            0x6000..=0x7FFF => {
                let off = (addr - 0x6000) as usize;
                self.wram[off]
            }
            0x8000..=0xFFFF => self
                .mapper
                .as_ref()
                .map(|m| m.borrow().read_prg(addr))
                .unwrap_or(0),
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => {
                let a = (addr as usize) & 0x07FF;
                self.ram[a] = val;
            }
            0x2000..=0x3FFF => {
                let reg = 0x2000 + (addr - 0x2000) % 8;
                // Log writes to PPU registers (0x2000..0x2007 and specifically 0x2006/0x2007)
                if log_ppu_writes() && reg >= 0x2000 && reg <= 0x2007 {
                    eprintln!(
                        "PPU WRITE: addr=0x{:04X} reg=0x{:04X} val=0x{:02X}",
                        addr, reg, val
                    );
                }
                // Forward writes to PPU registers
                self.ppu.write_register(reg, val);
            }
            0x4014 => {
                // OAM DMA: copy page (val * 0x100) into PPU OAM
                // Read the page into a temporary buffer first to avoid borrowing self immutably
                // while also mutably borrowing `ppu`.
                let base = (val as u16) << 8;
                let mut buf = [0u8; 256];
                for i in 0..256u16 {
                    buf[i as usize] = self.read(base.wrapping_add(i));
                }
                self.ppu.dma_oam_from_slice(&buf);
            }
            0x4000..=0x4017 => {
                // APU registers and controller strobe
                if (0x4000..=0x4007).contains(&addr) || addr == 0x4015 {
                    self.apu.write_register(addr, val);
                }

                // Controller strobe (0x4016): when bit0 is 1, latch current state; when 0, begin shifting.
                if addr == 0x4016 {
                    let st = (val & 1) != 0;
                    self.strobe.set(st);
                    if st {
                        self.controller_shift[0].set(self.controller_state[0]);
                        self.controller_shift[1].set(self.controller_state[1]);
                    }
                }
            }
            0x6000..=0x7FFF => {
                let off = (addr - 0x6000) as usize;
                self.wram[off] = val;
            }
            0x8000..=0xFFFF => {
                if let Some(m) = &mut self.mapper {
                    m.borrow_mut().write_prg(addr, val, &mut self.ppu);
                }
            }
            _ => {}
        }
    }
}
