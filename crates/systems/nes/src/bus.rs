use crate::apu::APU;
use crate::cartridge::{Cartridge, Mirroring};
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

#[derive(Debug)]
enum Mapper {
    Nrom(Nrom),
    Mmc3(Mmc3),
    Mmc1(Mmc1),
    Uxrom(Uxrom),
}

impl Mapper {
    fn from_cart(cart: Cartridge, ppu: &mut Ppu) -> Self {
        match cart.mapper {
            4 => Mapper::Mmc3(Mmc3::new(cart, ppu)),
            1 => Mapper::Mmc1(Mmc1::new(cart, ppu)),
            2 => Mapper::Uxrom(Uxrom::new(cart, ppu)),
            _ => Mapper::Nrom(Nrom::new(cart)),
        }
    }

    fn read_prg(&self, addr: u16) -> u8 {
        match self {
            Mapper::Nrom(m) => m.read_prg(addr),
            Mapper::Mmc3(m) => m.read_prg(addr),
            Mapper::Mmc1(m) => m.read_prg(addr),
            Mapper::Uxrom(m) => m.read_prg(addr),
        }
    }

    fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        match self {
            Mapper::Nrom(_) => {
                // NROM ignores PRG writes
                let _ = (addr, val, ppu);
            }
            Mapper::Mmc3(m) => m.write_prg(addr, val, ppu),
            Mapper::Mmc1(m) => m.write_prg(addr, val, ppu),
            Mapper::Uxrom(m) => m.write_prg(addr, val, ppu),
        }
    }

    fn prg_rom(&self) -> &[u8] {
        match self {
            Mapper::Nrom(m) => &m.prg_rom,
            Mapper::Mmc3(m) => &m.prg_rom,
            Mapper::Mmc1(m) => &m.prg_rom,
            Mapper::Uxrom(m) => &m.prg_rom,
        }
    }

    fn take_irq_pending(&mut self) -> bool {
        match self {
            Mapper::Nrom(_) => false,
            Mapper::Mmc3(m) => m.take_irq_pending(),
            Mapper::Mmc1(_) => false,
            Mapper::Uxrom(_) => false,
        }
    }

    fn notify_a12(&mut self, a12_high: bool) {
        if let Mapper::Mmc3(m) = self {
            m.notify_a12(a12_high);
        }
    }
}

#[derive(Debug)]
struct Nrom {
    prg_rom: Vec<u8>,
}

impl Nrom {
    fn new(cart: Cartridge) -> Self {
        Self {
            prg_rom: cart.prg_rom,
        }
    }

    fn read_prg(&self, addr: u16) -> u8 {
        let prg = &self.prg_rom;
        let len = prg.len();
        if len == 0 {
            return 0;
        }
        let off = if len == 0x4000 {
            (addr as usize - 0x8000) % 0x4000
        } else {
            (addr as usize - 0x8000) % len
        };
        prg[off]
    }
}

#[derive(Debug)]
struct Mmc3 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    bank_select: u8,
    bank_regs: [u8; 8],
    prg_banks: [usize; 4], // four 8KB banks mapped to $8000/$A000/$C000/$E000
    chr_banks: [usize; 8], // eight 1KB banks mapped to $0000-$1FFF
    prg_mode: bool,
    chr_mode: bool,
    irq_latch: u8,
    irq_counter: u8,
    irq_reload: bool,
    irq_enabled: bool,
    irq_pending: bool,
    last_a12: bool,
}

impl Mmc3 {
    fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        let mut m = Self {
            prg_rom: cart.prg_rom,
            chr_rom: cart.chr_rom,
            bank_select: 0,
            bank_regs: [0; 8],
            prg_banks: [0; 4],
            chr_banks: [0; 8],
            prg_mode: false,
            chr_mode: false,
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: false,
            last_a12: false,
        };
        m.apply_banks(ppu);
        // Respect initial mirroring from header until mapper writes override it.
        ppu.set_mirroring(cart.mirroring);
        m
    }

    fn prg_bank_count(&self) -> usize {
        std::cmp::max(1, self.prg_rom.len() / 0x2000)
    }

    fn chr_bank_count(&self) -> usize {
        std::cmp::max(1, self.chr_rom.len() / 0x0400)
    }

    fn apply_banks(&mut self, ppu: &mut Ppu) {
        // PRG banking
        let prg_count = self.prg_bank_count();
        let last = prg_count.saturating_sub(1);
        let second_last = prg_count.saturating_sub(2);
        let bank6 = (self.bank_regs[6] as usize) % prg_count;
        let bank7 = (self.bank_regs[7] as usize) % prg_count;

        if !self.prg_mode {
            self.prg_banks = [bank6, bank7, second_last, last];
        } else {
            self.prg_banks = [second_last, bank7, bank6, last];
        }

        // CHR banking (1KB units with two 2KB registers)
        let chr_count = self.chr_bank_count();
        let r0 = (self.bank_regs[0] & 0xFE) as usize % chr_count;
        let r1 = (self.bank_regs[1] & 0xFE) as usize % chr_count;
        let r2 = (self.bank_regs[2] as usize) % chr_count;
        let r3 = (self.bank_regs[3] as usize) % chr_count;
        let r4 = (self.bank_regs[4] as usize) % chr_count;
        let r5 = (self.bank_regs[5] as usize) % chr_count;

        if !self.chr_mode {
            self.chr_banks = [r0, r0 + 1, r1, r1 + 1, r2, r3, r4, r5];
        } else {
            self.chr_banks = [r2, r3, r4, r5, r0, r0 + 1, r1, r1 + 1];
        }

        self.update_chr_mapping(ppu);
    }

    fn update_chr_mapping(&self, ppu: &mut Ppu) {
        if ppu.chr.len() < 0x2000 {
            ppu.chr.resize(0x2000, 0);
        }

        // For CHR RAM carts, skip copying (PPU owns RAM). For CHR ROM, copy selected banks into 0x0000-0x1FFF view.
        if self.chr_rom.is_empty() {
            return;
        }

        for (i, bank) in self.chr_banks.iter().enumerate() {
            let dst_start = i * 0x0400;
            let src_start = bank.saturating_mul(0x0400);
            let src_end = src_start.saturating_add(0x0400);
            if src_end <= self.chr_rom.len() {
                ppu.chr[dst_start..dst_start + 0x0400]
                    .copy_from_slice(&self.chr_rom[src_start..src_end]);
            } else {
                // Out-of-range banks return 0s.
                for b in &mut ppu.chr[dst_start..dst_start + 0x0400] {
                    *b = 0;
                }
            }
        }
    }

    fn read_prg(&self, addr: u16) -> u8 {
        let bank = ((addr - 0x8000) / 0x2000) as usize;
        let offset = (addr as usize) & 0x1FFF;
        if bank >= self.prg_banks.len() {
            return 0;
        }
        let base = self.prg_banks[bank].saturating_mul(0x2000);
        let idx = base + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        match addr {
            0x8000..=0x9FFF => {
                if addr & 1 == 0 {
                    // Bank select
                    self.bank_select = val & 0x07;
                    self.prg_mode = (val & 0x40) != 0;
                    self.chr_mode = (val & 0x80) != 0;
                    self.apply_banks(ppu);
                } else {
                    // Bank data
                    self.bank_regs[self.bank_select as usize] = val;
                    self.apply_banks(ppu);
                }
            }
            0xA000..=0xBFFF => {
                if addr & 1 == 0 {
                    // Mirroring control: 0=vertical, 1=horizontal
                    let mir = if val & 1 == 0 {
                        Mirroring::Vertical
                    } else {
                        Mirroring::Horizontal
                    };
                    ppu.set_mirroring(mir);
                } else {
                    // PRG RAM protect (ignored)
                }
            }
            0xC000..=0xDFFF => {
                if addr & 1 == 0 {
                    self.irq_latch = val;
                } else {
                    self.irq_reload = true;
                }
            }
            0xE000..=0xFFFF => {
                if addr & 1 == 0 {
                    self.irq_enabled = false;
                    self.irq_pending = false; // disabling also clears pending
                } else {
                    self.irq_enabled = true;
                }
            }
            _ => {}
        }
    }

    fn notify_a12(&mut self, a12_high: bool) {
        if !self.last_a12 && a12_high {
            // Rising edge clocks the counter per MMC3 spec.
            if self.irq_reload || self.irq_counter == 0 {
                self.irq_counter = self.irq_latch;
                self.irq_reload = false;
            } else {
                self.irq_counter = self.irq_counter.saturating_sub(1);
                if self.irq_counter == 0 && self.irq_enabled {
                    self.irq_pending = true;
                }
            }
        }
        self.last_a12 = a12_high;
    }

    fn take_irq_pending(&mut self) -> bool {
        let was = self.irq_pending;
        self.irq_pending = false;
        was
    }
}

#[derive(Debug)]
struct Mmc1 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    shift_reg: u8,
    write_count: u8,
    control: u8,
    prg_bank: u8,
    chr_bank0: u8,
    chr_bank1: u8,
    prg_banks: [usize; 2], // two 16KB banks at $8000 and $C000
    chr_banks: [usize; 2], // two 4KB banks at $0000 and $1000
}

#[derive(Debug)]
struct Uxrom {
    prg_rom: Vec<u8>,
    bank_select: u8,
}

impl Uxrom {
    fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        // UxROM uses fixed mirroring from the header.
        ppu.set_mirroring(cart.mirroring);
        Self {
            prg_rom: cart.prg_rom,
            bank_select: 0,
        }
    }

    fn prg_bank_count(&self) -> usize {
        std::cmp::max(1, self.prg_rom.len() / 0x4000)
    }

    fn read_prg(&self, addr: u16) -> u8 {
        let bank = if addr < 0xC000 {
            (self.bank_select as usize) % self.prg_bank_count()
        } else {
            // Fixed last bank at $C000-$FFFF.
            self.prg_bank_count().saturating_sub(1)
        };
        let offset = (addr as usize) & 0x3FFF;
        let idx = bank.saturating_mul(0x4000) + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    fn write_prg(&mut self, addr: u16, val: u8, _ppu: &mut Ppu) {
        if (0x8000..=0xFFFF).contains(&addr) {
            // Select 16KB bank for $8000-$BFFF; upper bits ignored beyond available banks.
            self.bank_select = val & 0x0F;
        }
    }
}

impl Mmc1 {
    fn new(cart: Cartridge, ppu: &mut Ppu) -> Self {
        let mut m = Self {
            prg_rom: cart.prg_rom,
            chr_rom: cart.chr_rom,
            shift_reg: 0x10,
            write_count: 0,
            control: 0x0C, // default: 16KB PRG switching, 8KB CHR
            prg_bank: 0,
            chr_bank0: 0,
            chr_bank1: 0,
            prg_banks: [0, 0],
            chr_banks: [0, 1],
        };
        // Respect header mirroring until mapper writes override it.
        ppu.set_mirroring(cart.mirroring);
        m.apply_banks(ppu);
        m
    }

    fn prg_bank_count(&self) -> usize {
        std::cmp::max(1, self.prg_rom.len() / 0x4000)
    }

    fn chr_bank_count(&self) -> usize {
        std::cmp::max(1, self.chr_rom.len() / 0x1000)
    }

    fn apply_banks(&mut self, ppu: &mut Ppu) {
        let prg_count = self.prg_bank_count();
        let last = prg_count.saturating_sub(1);
        let prg_mode = (self.control >> 2) & 0x03;
        let select = (self.prg_bank & 0x1F) as usize % prg_count;

        self.prg_banks = match prg_mode {
            0 | 1 => {
                // 32KB mode: even bank paired with next bank
                let even = (self.prg_bank & 0x1E) as usize % prg_count;
                [even, (even + 1) % prg_count]
            }
            2 => [0, select],    // fix first, swap upper
            _ => [select, last], // swap lower, fix last
        };

        let chr_mode = (self.control >> 4) & 1 != 0;
        let chr_count = self.chr_bank_count();
        if !chr_mode {
            // 8KB mode
            let bank = (self.chr_bank0 & 0x1E) as usize % chr_count;
            self.chr_banks = [bank, (bank + 1) % chr_count];
        } else {
            self.chr_banks = [
                (self.chr_bank0 as usize) % chr_count,
                (self.chr_bank1 as usize) % chr_count,
            ];
        }

        // Mirroring: 0=single screen low, 1=single screen high, 2=vertical, 3=horizontal
        let mir = match self.control & 0x03 {
            0 => Mirroring::SingleScreenLower,
            1 => Mirroring::SingleScreenUpper,
            2 => Mirroring::Vertical,
            _ => Mirroring::Horizontal,
        };
        ppu.set_mirroring(mir);

        self.update_chr_mapping(ppu);
    }

    fn update_chr_mapping(&self, ppu: &mut Ppu) {
        if ppu.chr.len() < 0x2000 {
            ppu.chr.resize(0x2000, 0);
        }

        // CHR RAM carts skip copying since PPU owns the RAM view.
        if self.chr_rom.is_empty() {
            return;
        }

        for (i, bank) in self.chr_banks.iter().enumerate() {
            let dst_start = i * 0x1000;
            let src_start = bank.saturating_mul(0x1000);
            let src_end = src_start.saturating_add(0x1000);
            if src_end <= self.chr_rom.len() {
                ppu.chr[dst_start..dst_start + 0x1000]
                    .copy_from_slice(&self.chr_rom[src_start..src_end]);
            } else {
                for b in &mut ppu.chr[dst_start..dst_start + 0x1000] {
                    *b = 0;
                }
            }
        }
    }

    fn latch_write(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        if val & 0x80 != 0 {
            // Reset shift register
            self.shift_reg = 0x10;
            self.write_count = 0;
            self.control |= 0x0C;
            self.apply_banks(ppu);
            return;
        }

        // Collect 5 bits, LSB first.
        self.shift_reg = (self.shift_reg >> 1) | ((val & 1) << 4);
        self.write_count = self.write_count.saturating_add(1);

        if self.write_count < 5 {
            return;
        }

        let data = self.shift_reg & 0x1F;
        let target = (addr >> 13) & 0x03; // 0: control, 1: CHR0, 2: CHR1, 3: PRG
        match target {
            0 => self.control = data | 0x00,
            1 => self.chr_bank0 = data,
            2 => self.chr_bank1 = data,
            3 => self.prg_bank = data,
            _ => {}
        }

        self.shift_reg = 0x10;
        self.write_count = 0;
        self.apply_banks(ppu);
    }

    fn read_prg(&self, addr: u16) -> u8 {
        let bank = ((addr - 0x8000) / 0x4000) as usize;
        let offset = (addr as usize) & 0x3FFF;
        let prg_bank = *self.prg_banks.get(bank).unwrap_or(&0);
        let idx = prg_bank.saturating_mul(0x4000) + offset;
        self.prg_rom.get(idx).copied().unwrap_or(0)
    }

    fn write_prg(&mut self, addr: u16, val: u8, ppu: &mut Ppu) {
        if (0x8000..=0xFFFF).contains(&addr) {
            self.latch_write(addr, val, ppu);
        }
    }
}
