//! Sharp SM510 4-bit microcontroller emulation.
//!
//! The SM510 is a 4-bit MCU used in Nintendo's Game & Watch handhelds.
//! It runs at 32.768 kHz from a watch crystal and has:
//! - 4-bit accumulator, carry flag
//! - 12-bit program counter with single-level stack
//! - 128 × 4-bit RAM (addressed via BL/BM registers)
//! - Up to 4 KB ROM
//! - 15-bit frequency divider
//! - LCD segment driver and melody generator

/// SM510 CPU state
pub struct Sm510 {
    // === Registers ===
    /// 4-bit accumulator
    pub acc: u8,
    /// Carry flag
    pub carry: bool,
    /// Program counter (12-bit, page:offset)
    pub pc: u16,
    /// Previous PC (for debugging)
    pub prev_pc: u16,
    /// Single-level stack (stores return address for TML)
    pub stack: u16,
    /// BL register (4-bit, RAM address low)
    pub bl: u8,
    /// BM register (2-bit base, RAM address mid)
    pub bm: u8,
    /// SBM flag (extends BM by 1 bit for upper RAM access)
    pub sbm: bool,
    /// Skip flag (skip next instruction)
    pub skip: bool,

    // === Memory ===
    /// Program ROM (up to 4 KB)
    pub rom: Vec<u8>,
    /// 128 × 4-bit RAM (stored as u8, only low nibble used)
    pub ram: [u8; 128],

    // === I/O ===
    /// K input port (4-bit, active high)
    pub input_k: u8,
    /// BA input (1-bit)
    pub input_ba: bool,
    /// B input (1-bit)
    pub input_b: bool,
    /// S output register (4-bit, used for LCD and input matrix)
    pub output_s: u8,
    /// R output register (4-bit)
    pub output_r: u8,
    /// L output register (segment latch)
    pub output_l: u8,
    /// X output register
    pub output_x: u8,
    /// BP (beta plate) output
    pub bp: u8,
    /// Alpha (alarm) output state
    pub alpha: bool,

    // === Frequency divider ===
    /// 15-bit frequency divider (clocked at 32768 Hz)
    pub divider: u16,
    /// F1 flag (triggered from divider, ~64 Hz)
    pub f1_flag: bool,
    /// F4 flag (triggered from divider, ~16 Hz)
    pub f4_flag: bool,

    // === Melody / buzzer ===
    /// Melody generator enabled
    pub melody_enabled: bool,
    /// Melody step counter
    pub melody_step: u8,
    /// Buzzer output state
    pub buzzer_active: bool,

    // === Status ===
    /// Halted (CEND - standby mode)
    pub halted: bool,
    /// Total cycles executed
    pub cycles: u64,
    /// Pending immediate clock (for 5-step mode equivalent)
    pub pending_immediate_clock: bool,

    // === Internal ===
    /// Current opcode being executed
    op: u8,
    /// Parameter byte for 2-byte instructions
    param: u8,
    /// ROM address mask
    prg_mask: u16,

    // === Controller state ===
    /// Bits 0-3: directional (L, R, U, D)
    /// Bits 4-7: action (Game A, Game B, Time, Alarm)
    /// Bit 8: ACL
    pub controller_state: u16,

    // === Keyboard mapping (from .mgw ROM) ===
    /// Optional keyboard mapping from .mgw ROM data.
    /// keyboard[0..7] = S1..S8: each u32 has 4 bytes encoding K1-K4 button masks
    /// keyboard[8] = BA direct button mask, keyboard[9] = B direct button mask
    pub keyboard_mapping: Option<[u32; 10]>,

    /// Raw button state for keyboard-mapped input.
    /// Uses .mgw button encoding: LEFT=0x01, UP=0x02, RIGHT=0x04, DOWN=0x08,
    /// A=0x10, B=0x20, TIME=0x40, GAME=0x80
    pub pressed_buttons: u8,
}

impl Sm510 {
    /// Create a new SM510 with empty ROM
    pub fn new() -> Self {
        Self {
            acc: 0,
            carry: false,
            pc: 0,
            prev_pc: 0,
            stack: 0,
            bl: 0,
            bm: 0,
            sbm: false,
            skip: false,
            rom: vec![0; 4096],
            ram: [0; 128],
            input_k: 0,
            input_ba: false,
            input_b: false,
            output_s: 0,
            output_r: 0,
            output_l: 0,
            output_x: 0,
            bp: 0,
            alpha: false,
            divider: 0,
            f1_flag: false,
            f4_flag: false,
            melody_enabled: false,
            melody_step: 0,
            buzzer_active: false,
            halted: false,
            cycles: 0,
            pending_immediate_clock: false,
            op: 0,
            param: 0,
            prg_mask: 0xFFF,
            controller_state: 0,
            keyboard_mapping: None,
            pressed_buttons: 0,
        }
    }

    /// Load ROM data
    pub fn load_rom(&mut self, data: &[u8]) {
        let len = data.len().min(4096);
        self.rom[..len].copy_from_slice(&data[..len]);
        // Set program mask based on ROM size
        self.prg_mask = match len {
            0..=256 => 0xFF,
            257..=512 => 0x1FF,
            513..=1024 => 0x3FF,
            1025..=2048 => 0x7FF,
            _ => 0xFFF,
        };
    }

    /// Reset CPU to initial state
    pub fn reset(&mut self) {
        self.acc = 0;
        self.carry = false;
        self.pc = 0;
        self.prev_pc = 0;
        self.stack = 0;
        self.bl = 0;
        self.bm = 0;
        self.sbm = false;
        self.skip = false;
        self.ram = [0; 128];
        self.input_k = 0;
        self.input_ba = false;
        self.input_b = false;
        self.output_s = 0;
        self.output_r = 0;
        self.output_l = 0;
        self.output_x = 0;
        self.bp = 0;
        self.alpha = false;
        self.divider = 0;
        self.f1_flag = false;
        self.f4_flag = false;
        self.melody_enabled = false;
        self.melody_step = 0;
        self.buzzer_active = false;
        self.halted = false;
        self.cycles = 0;
        self.pending_immediate_clock = false;
        self.op = 0;
        self.param = 0;
    }

    // === Memory access ===

    /// Calculate RAM address from BM, BL, and SBM flag
    fn ram_address(&self) -> usize {
        let bm_effective = if self.sbm {
            (self.bm & 0x3) | 0x4
        } else {
            self.bm & 0x3
        };
        ((bm_effective as usize) << 4 | (self.bl as usize) & 0xF) & 0x7F
    }

    /// Read 4-bit value from RAM at current BL/BM address
    fn ram_read(&self) -> u8 {
        self.ram[self.ram_address()] & 0xF
    }

    /// Write 4-bit value to RAM at current BL/BM address
    fn ram_write(&mut self, val: u8) {
        let addr = self.ram_address();
        self.ram[addr] = val & 0xF;
    }

    /// Read byte from ROM at given address
    fn rom_read(&self, addr: u16) -> u8 {
        let addr = (addr & self.prg_mask) as usize;
        if addr < self.rom.len() {
            self.rom[addr]
        } else {
            0
        }
    }

    /// Increment PC within current page (wraps at page boundary)
    fn increment_pc(&mut self) {
        let page = self.pc & !0x3F;
        let offset = (self.pc & 0x3F) + 1;
        self.pc = page | (offset & 0x3F);
    }

    /// Check if opcode is a 2-byte instruction
    fn is_2byte(op: u8) -> bool {
        // TL (0x70-0x7B), TML (0x7C-0x7F), LBL (0x5F), PRE (0x6C)
        matches!(op, 0x70..=0x7F | 0x5F | 0x6C)
    }

    /// Push PC onto stack
    fn push_stack(&mut self) {
        self.stack = self.pc;
    }

    /// Pop stack into PC
    fn pop_stack(&mut self) {
        self.pc = self.stack & self.prg_mask;
    }

    // === Input matrix ===

    /// Update K input based on S output and controller state.
    ///
    /// When a keyboard mapping from an .mgw ROM is available, uses the per-game
    /// wiring table to map physical buttons to the SM510 S/K input matrix.
    /// Otherwise falls back to a simple hardcoded mapping.
    fn update_input(&mut self) {
        if let Some(ref kbd) = self.keyboard_mapping {
            // .mgw keyboard-mapped input
            let buttons = self.pressed_buttons;
            let mut k = 0u8;

            for (s, &mapping) in kbd.iter().enumerate().take(8) {
                if self.output_s & (1 << s) != 0 {
                    // K1: byte 0 (bits 0-7) = button mask for K1
                    if (mapping as u8) & buttons != 0 {
                        k |= 0x1;
                    }
                    // K2: byte 1 (bits 8-15)
                    if ((mapping >> 8) as u8) & buttons != 0 {
                        k |= 0x2;
                    }
                    // K3: byte 2 (bits 16-23)
                    if ((mapping >> 16) as u8) & buttons != 0 {
                        k |= 0x4;
                    }
                    // K4: byte 3 (bits 24-31)
                    if ((mapping >> 24) as u8) & buttons != 0 {
                        k |= 0x8;
                    }
                }
            }

            self.input_k = k & 0xF;
            // BA and B are direct inputs (not matrix-scanned)
            self.input_ba = (kbd[8] as u8) & buttons != 0;
            self.input_b = (kbd[9] as u8) & buttons != 0;
        } else {
            // Fallback: hardcoded mapping for raw ROMs
            let mut k = 0u8;

            // S bit 0 → directional buttons (controller bits 0-3)
            if self.output_s & 0x1 != 0 {
                k |= (self.controller_state & 0xF) as u8;
            }
            // S bit 1 → action buttons (controller bits 4-7)
            if self.output_s & 0x2 != 0 {
                k |= ((self.controller_state >> 4) & 0xF) as u8;
            }
            // S bit 2 → ACL (controller bit 8)
            if self.output_s & 0x4 != 0 {
                k |= ((self.controller_state >> 8) & 0x1) as u8;
            }
            // S bit 3 → additional inputs
            if self.output_s & 0x8 != 0 {
                k |= ((self.controller_state >> 8) & 0xF) as u8;
            }

            self.input_k = k & 0xF;
            self.input_ba = (self.controller_state & 0x10) != 0;
            self.input_b = (self.controller_state & 0x20) != 0;
        }
    }

    // === Frequency divider ===

    /// Clock the frequency divider (called once per CPU cycle at 32768 Hz)
    fn clock_divider(&mut self) {
        let old_div = self.divider;
        self.divider = (self.divider + 1) & 0x7FFF;

        // F1: toggles at divider bit 8 (32768 / 512 = 64 Hz)
        if (old_div ^ self.divider) & 0x100 != 0 && self.divider & 0x100 != 0 {
            self.f1_flag = true;
        }

        // F4: toggles at divider bit 11 (32768 / 4096 = 8 Hz)
        if (old_div ^ self.divider) & 0x800 != 0 && self.divider & 0x800 != 0 {
            self.f4_flag = true;
        }
    }

    // === Instruction execution ===

    /// Execute one instruction. Returns cycles consumed (always 1).
    pub fn step(&mut self) -> u32 {
        if self.halted {
            // In standby mode, still clock divider
            self.clock_divider();
            self.cycles += 1;

            // Wake up on any K input
            if self.input_k != 0 || self.input_ba || self.input_b {
                self.halted = false;
            }
            return 1;
        }

        // Update input matrix before execution
        self.update_input();

        // Fetch opcode
        self.prev_pc = self.pc;
        self.op = self.rom_read(self.pc);
        self.increment_pc();

        // Fetch parameter for 2-byte instructions
        if Self::is_2byte(self.op) {
            self.param = self.rom_read(self.pc);
            self.increment_pc();
        }

        // Handle skip
        if self.skip {
            self.skip = false;
            // Instruction was fetched but not executed
            self.clock_divider();
            self.cycles += 1;
            return 1;
        }

        // Decode and execute
        self.execute();

        // Clock divider
        self.clock_divider();
        self.cycles += 1;

        1
    }

    /// Decode and execute the current opcode
    fn execute(&mut self) {
        match self.op {
            // 0x00: SKIP - set skip flag
            0x00 => self.op_skip(),
            // 0x01: ATBP - ACC to beta plate output
            0x01 => self.op_atbp(),
            // 0x02: SBM - set BM high flag
            0x02 => self.op_sbm(),
            // 0x03: ATPL - ACC to PL (low PC bits)
            0x03 => self.op_atpl(),
            // 0x04-0x07: RM r - reset RAM bit r
            0x04..=0x07 => self.op_rm(),
            // 0x08: ADD - ACC = ACC + RAM[addr]
            0x08 => self.op_add(),
            // 0x09: ADD11 - ACC = ACC + RAM[addr] + Carry
            0x09 => self.op_add11(),
            // 0x0A: COMA - complement ACC
            0x0A => self.op_coma(),
            // 0x0B: EXBLA - exchange BL and ACC
            0x0B => self.op_exbla(),
            // 0x0C-0x0F: SM r - set RAM bit r
            0x0C..=0x0F => self.op_sm(),
            // 0x10-0x13: EXC - exchange ACC with RAM
            0x10..=0x13 => self.op_exc(),
            // 0x14-0x17: EXCI - exchange ACC with RAM, increment BL
            0x14..=0x17 => self.op_exci(),
            // 0x18-0x1B: LDA - load ACC from RAM
            0x18..=0x1B => self.op_lda(),
            // 0x1C-0x1F: EXCD - exchange ACC with RAM, decrement BL
            0x1C..=0x1F => self.op_excd(),
            // 0x20-0x2F: LAX n - load ACC with immediate
            0x20..=0x2F => self.op_lax(),
            // 0x30-0x3F: ADX n - add immediate to ACC
            0x30..=0x3F => self.op_adx(),
            // 0x40-0x4F: LB n - load BL with immediate
            0x40..=0x4F => self.op_lb(),
            // 0x50-0x5F: various test and control
            0x50 => self.op_kta(),
            0x51 => self.op_tb(),
            0x52 => self.op_tc(),
            0x53 => self.op_tam(),
            0x54..=0x57 => self.op_tmi(),
            0x58 => self.op_tis(),
            0x59 => self.op_atl(),
            0x5A => self.op_ta0(),
            0x5B => self.op_tabl(),
            0x5C => self.op_atx(),
            0x5D => self.op_cend(),
            0x5E => self.op_tal(),
            0x5F => self.op_lbl(),
            // 0x60-0x6F: control and flag operations
            0x60 => self.op_atfc(),
            0x61 => self.op_atr(),
            0x62 => self.op_wr(),
            0x63 => self.op_ws(),
            0x64 => self.op_incb(),
            0x65 => self.op_idiv(),
            0x66 => self.op_rc(),
            0x67 => self.op_sc(),
            0x68 => self.op_tf1(),
            0x69 => self.op_tf4(),
            0x6A => self.op_rtn0(),
            0x6B => self.op_rtn1(),
            0x6C => self.op_pre(),
            0x6D => self.op_sme(),
            0x6E => self.op_rme(),
            0x6F => self.op_tme(),
            // 0x70-0x7B: TL - transfer long (jump)
            0x70..=0x7B => self.op_tl(),
            // 0x7C-0x7F: TML - transfer & mark long (call)
            0x7C..=0x7F => self.op_tml(),
            // 0x80-0xFF: T - short branch within page
            0x80..=0xFF => self.op_t(),
        }
    }

    // ================================================
    // Instruction implementations
    // ================================================

    /// SKIP (0x00): Set skip flag (next instruction is skipped)
    fn op_skip(&mut self) {
        self.skip = true;
    }

    /// ATBP (0x01): ACC to Beta Plate output
    fn op_atbp(&mut self) {
        self.bp = self.acc & 0x1;
    }

    /// SBM (0x02): Set BM high flag (extends RAM addressing)
    fn op_sbm(&mut self) {
        self.sbm = true;
    }

    /// ATPL (0x03): ACC to PL (set low 4 bits of PC offset)
    fn op_atpl(&mut self) {
        self.pc = (self.pc & !0xF) | (self.acc as u16 & 0xF);
    }

    /// RM r (0x04-0x07): Reset bit r of RAM[BM:BL]
    fn op_rm(&mut self) {
        let bit = 1u8 << (self.op & 0x3);
        let val = self.ram_read() & !bit;
        self.ram_write(val);
    }

    /// ADD (0x08): ACC = ACC + RAM[BM:BL], update carry
    fn op_add(&mut self) {
        let result = self.acc as u16 + self.ram_read() as u16;
        self.carry = result > 0xF;
        self.acc = (result & 0xF) as u8;
    }

    /// ADD11 (0x09): ACC = ACC + RAM[BM:BL] + Carry, update carry
    fn op_add11(&mut self) {
        let c = if self.carry { 1u16 } else { 0 };
        let result = self.acc as u16 + self.ram_read() as u16 + c;
        self.carry = result > 0xF;
        self.acc = (result & 0xF) as u8;
    }

    /// COMA (0x0A): Complement ACC (ones' complement)
    fn op_coma(&mut self) {
        self.acc ^= 0xF;
    }

    /// EXBLA (0x0B): Exchange BL and ACC
    fn op_exbla(&mut self) {
        let tmp = self.bl;
        self.bl = self.acc & 0xF;
        self.acc = tmp & 0xF;
    }

    /// SM r (0x0C-0x0F): Set bit r of RAM[BM:BL]
    fn op_sm(&mut self) {
        let bit = 1u8 << (self.op & 0x3);
        let val = self.ram_read() | bit;
        self.ram_write(val);
    }

    /// EXC (0x10-0x13): Exchange ACC with RAM[BM:BL]
    fn op_exc(&mut self) {
        let ram_val = self.ram_read();
        self.ram_write(self.acc);
        self.acc = ram_val;
    }

    /// EXCI (0x14-0x17): Exchange ACC with RAM[BM:BL], then increment BL
    fn op_exci(&mut self) {
        self.op_exc();
        self.bl = (self.bl + 1) & 0xF;
        self.skip = self.bl == 0; // Skip on overflow
    }

    /// LDA (0x18-0x1B): Load ACC from RAM[BM:BL]
    fn op_lda(&mut self) {
        self.acc = self.ram_read();
    }

    /// EXCD (0x1C-0x1F): Exchange ACC with RAM[BM:BL], then decrement BL
    fn op_excd(&mut self) {
        self.op_exc();
        self.bl = (self.bl.wrapping_sub(1)) & 0xF;
        self.skip = self.bl == 0xF; // Skip on underflow
    }

    /// LAX n (0x20-0x2F): Load ACC with immediate n
    fn op_lax(&mut self) {
        self.acc = self.op & 0xF;
    }

    /// ADX n (0x30-0x3F): Add immediate n to ACC, skip on carry
    fn op_adx(&mut self) {
        let result = self.acc as u16 + (self.op & 0xF) as u16;
        self.skip = result > 0xF;
        self.acc = (result & 0xF) as u8;
    }

    /// LB n (0x40-0x4F): Load BL with immediate n
    fn op_lb(&mut self) {
        self.bl = self.op & 0xF;
    }

    /// KTA (0x50): K input port to ACC
    fn op_kta(&mut self) {
        self.acc = self.input_k & 0xF;
    }

    /// TB (0x51): Test Beta Plate, skip if non-zero
    fn op_tb(&mut self) {
        self.skip = self.bp != 0;
    }

    /// TC (0x52): Test Carry, skip if set
    fn op_tc(&mut self) {
        self.skip = self.carry;
    }

    /// TAM (0x53): Test ACC AND RAM, skip if non-zero
    fn op_tam(&mut self) {
        self.skip = (self.acc & self.ram_read()) != 0;
    }

    /// TMI r (0x54-0x57): Test Memory bit r, skip if set
    fn op_tmi(&mut self) {
        let bit = 1u8 << (self.op & 0x3);
        self.skip = (self.ram_read() & bit) != 0;
    }

    /// TIS (0x58): Test divider output, skip if set
    fn op_tis(&mut self) {
        // Test if 1-second timer has elapsed (divider bit 14)
        self.skip = self.divider & 0x4000 != 0;
        // Clear the tested flag
        if self.skip {
            self.divider &= !0x4000;
        }
    }

    /// ATL (0x59): ACC to L output (segment latch)
    fn op_atl(&mut self) {
        self.output_l = self.acc & 0xF;
    }

    /// TA0 (0x5A): Test ACC == 0, skip if true
    fn op_ta0(&mut self) {
        self.skip = self.acc == 0;
    }

    /// TABL (0x5B): Transfer ACC to BL
    fn op_tabl(&mut self) {
        self.bl = self.acc & 0xF;
    }

    /// ATX (0x5C): ACC to X output
    fn op_atx(&mut self) {
        self.output_x = self.acc & 0xF;
    }

    /// CEND (0x5D): Clock END - enter standby mode
    fn op_cend(&mut self) {
        self.halted = true;
    }

    /// TAL (0x5E): Test Alpha (alarm) output, skip if set
    fn op_tal(&mut self) {
        self.skip = self.alpha;
    }

    /// LBL (0x5F): 2-byte: Load BL and BM from parameter
    fn op_lbl(&mut self) {
        self.bl = (self.param >> 4) & 0xF;
        self.bm = self.param & 0x3;
        self.sbm = false; // LBL clears SBM
    }

    /// ATFC (0x60): ACC bit 0 to Carry
    fn op_atfc(&mut self) {
        self.carry = (self.acc & 1) != 0;
    }

    /// ATR (0x61): ACC to R output
    fn op_atr(&mut self) {
        self.output_r = self.acc & 0xF;
    }

    /// WR (0x62): Write R output
    fn op_wr(&mut self) {
        // Triggers R output to LCD driver
        // (R output already set by ATR)
    }

    /// WS (0x63): Write S output (segment select / input matrix column)
    fn op_ws(&mut self) {
        self.output_s = self.acc & 0xF;
        // Update input matrix after S change
        self.update_input();
    }

    /// INCB (0x64): Increment BL, skip on overflow (BL == 0 after increment)
    fn op_incb(&mut self) {
        self.bl = (self.bl + 1) & 0xF;
        self.skip = self.bl == 0;
    }

    /// IDIV (0x65): Reset frequency divider
    fn op_idiv(&mut self) {
        self.divider = 0;
        self.f1_flag = false;
        self.f4_flag = false;
    }

    /// RC (0x66): Reset Carry flag
    fn op_rc(&mut self) {
        self.carry = false;
    }

    /// SC (0x67): Set Carry flag
    fn op_sc(&mut self) {
        self.carry = true;
    }

    /// TF1 (0x68): Test F1 divider flag, skip if set
    fn op_tf1(&mut self) {
        self.skip = self.f1_flag;
        self.f1_flag = false; // Clear after test
    }

    /// TF4 (0x69): Test F4 divider flag, skip if set
    fn op_tf4(&mut self) {
        self.skip = self.f4_flag;
        self.f4_flag = false; // Clear after test
    }

    /// RTN0 (0x6A): Return from subroutine (pop stack to PC)
    fn op_rtn0(&mut self) {
        self.pop_stack();
    }

    /// RTN1 (0x6B): Return from subroutine and skip
    fn op_rtn1(&mut self) {
        self.pop_stack();
        self.skip = true;
    }

    /// PRE (0x6C): 2-byte: Preset (load parameter to melody/output)
    fn op_pre(&mut self) {
        self.melody_step = self.param;
    }

    /// SME (0x6D): Set Melody Enable
    fn op_sme(&mut self) {
        self.melody_enabled = true;
        self.buzzer_active = true;
    }

    /// RME (0x6E): Reset Melody Enable
    fn op_rme(&mut self) {
        self.melody_enabled = false;
        self.buzzer_active = false;
    }

    /// TME (0x6F): Test Melody Enable, skip if enabled
    fn op_tme(&mut self) {
        self.skip = self.melody_enabled;
    }

    /// TL (0x70-0x7B): 2-byte: Transfer Long (jump)
    fn op_tl(&mut self) {
        let page = (self.op & 0xF) as u16;
        let offset = (self.param & 0x3F) as u16;
        self.pc = ((page << 6) | offset) & self.prg_mask;
    }

    /// TML (0x7C-0x7F): 2-byte: Transfer & Mark Long (call, pushes return address)
    fn op_tml(&mut self) {
        self.push_stack();
        let page = (self.op & 0x3) as u16;
        let offset = (self.param & 0x3F) as u16;
        self.pc = ((page << 6) | offset) & self.prg_mask;
    }

    /// T n (0x80-0xFF): Short branch within current page
    fn op_t(&mut self) {
        let offset = self.op & 0x3F;
        self.pc = (self.pc & !0x3F) | (offset as u16);
    }
}

impl Default for Sm510 {
    fn default() -> Self {
        Self::new()
    }
}

// === Disassembly support ===

impl Sm510 {
    /// Disassemble a single instruction at the given ROM address.
    /// Returns (mnemonic, byte_count)
    pub fn disassemble(rom: &[u8], addr: u16) -> (String, usize) {
        let addr_usize = addr as usize;
        if addr_usize >= rom.len() {
            return ("???".to_string(), 1);
        }

        let op = rom[addr_usize];
        let has_param = Self::is_2byte(op);
        let param = if has_param && addr_usize + 1 < rom.len() {
            rom[addr_usize + 1]
        } else {
            0
        };
        let len = if has_param { 2 } else { 1 };

        let mnemonic = match op {
            0x00 => "SKIP".to_string(),
            0x01 => "ATBP".to_string(),
            0x02 => "SBM".to_string(),
            0x03 => "ATPL".to_string(),
            0x04..=0x07 => format!("RM {}", op & 0x3),
            0x08 => "ADD".to_string(),
            0x09 => "ADD11".to_string(),
            0x0A => "COMA".to_string(),
            0x0B => "EXBLA".to_string(),
            0x0C..=0x0F => format!("SM {}", op & 0x3),
            0x10..=0x13 => format!("EXC {}", op & 0x3),
            0x14..=0x17 => format!("EXCI {}", op & 0x3),
            0x18..=0x1B => format!("LDA {}", op & 0x3),
            0x1C..=0x1F => format!("EXCD {}", op & 0x3),
            0x20..=0x2F => format!("LAX ${:X}", op & 0xF),
            0x30..=0x3F => format!("ADX ${:X}", op & 0xF),
            0x40..=0x4F => format!("LB ${:X}", op & 0xF),
            0x50 => "KTA".to_string(),
            0x51 => "TB".to_string(),
            0x52 => "TC".to_string(),
            0x53 => "TAM".to_string(),
            0x54..=0x57 => format!("TMI {}", op & 0x3),
            0x58 => "TIS".to_string(),
            0x59 => "ATL".to_string(),
            0x5A => "TA0".to_string(),
            0x5B => "TABL".to_string(),
            0x5C => "ATX".to_string(),
            0x5D => "CEND".to_string(),
            0x5E => "TAL".to_string(),
            0x5F => format!("LBL ${:02X}", param),
            0x60 => "ATFC".to_string(),
            0x61 => "ATR".to_string(),
            0x62 => "WR".to_string(),
            0x63 => "WS".to_string(),
            0x64 => "INCB".to_string(),
            0x65 => "IDIV".to_string(),
            0x66 => "RC".to_string(),
            0x67 => "SC".to_string(),
            0x68 => "TF1".to_string(),
            0x69 => "TF4".to_string(),
            0x6A => "RTN0".to_string(),
            0x6B => "RTN1".to_string(),
            0x6C => format!("PRE ${:02X}", param),
            0x6D => "SME".to_string(),
            0x6E => "RME".to_string(),
            0x6F => "TME".to_string(),
            0x70..=0x7B => {
                let target = ((op as u16 & 0xF) << 6) | (param as u16 & 0x3F);
                format!("TL ${:03X}", target)
            }
            0x7C..=0x7F => {
                let target = ((op as u16 & 0x3) << 6) | (param as u16 & 0x3F);
                format!("TML ${:03X}", target)
            }
            0x80..=0xFF => format!("T ${:02X}", op & 0x3F),
        };

        (mnemonic, len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cpu() {
        let cpu = Sm510::new();
        assert_eq!(cpu.acc, 0);
        assert_eq!(cpu.pc, 0);
        assert!(!cpu.carry);
        assert_eq!(cpu.bl, 0);
        assert_eq!(cpu.bm, 0);
        assert!(!cpu.sbm);
        assert!(!cpu.skip);
    }

    #[test]
    fn test_ram_addressing() {
        let mut cpu = Sm510::new();
        cpu.bl = 5;
        cpu.bm = 2;
        cpu.sbm = false;
        assert_eq!(cpu.ram_address(), (2 << 4) | 5); // 37

        cpu.sbm = true;
        assert_eq!(cpu.ram_address(), (6 << 4) | 5); // 101
    }

    #[test]
    fn test_ram_read_write() {
        let mut cpu = Sm510::new();
        cpu.bl = 3;
        cpu.bm = 1;
        cpu.ram_write(0xA);
        assert_eq!(cpu.ram_read(), 0xA);
        // Verify nibble masking
        cpu.ram_write(0xFF);
        assert_eq!(cpu.ram_read(), 0xF);
    }

    #[test]
    fn test_skip_instruction() {
        let mut cpu = Sm510::new();
        // ROM: SKIP, LAX 5, LAX 7
        cpu.rom[0] = 0x00; // SKIP
        cpu.rom[1] = 0x25; // LAX 5
        cpu.rom[2] = 0x27; // LAX 7

        cpu.step(); // Execute SKIP
        assert!(cpu.skip);

        cpu.step(); // LAX 5 is skipped
        assert_eq!(cpu.acc, 0); // ACC unchanged

        cpu.step(); // Execute LAX 7
        assert_eq!(cpu.acc, 7);
    }

    #[test]
    fn test_lax() {
        let mut cpu = Sm510::new();
        cpu.rom[0] = 0x2A; // LAX $A
        cpu.step();
        assert_eq!(cpu.acc, 0xA);
    }

    #[test]
    fn test_adx() {
        let mut cpu = Sm510::new();
        cpu.acc = 5;
        cpu.rom[0] = 0x33; // ADX 3
        cpu.step();
        assert_eq!(cpu.acc, 8);
        assert!(!cpu.skip); // No overflow

        // Test with overflow
        cpu.acc = 0xE;
        cpu.pc = 0;
        cpu.rom[0] = 0x33; // ADX 3
        cpu.step();
        assert_eq!(cpu.acc, 1); // (0xE + 3) & 0xF = 1
        assert!(cpu.skip); // Overflow
    }

    #[test]
    fn test_lb() {
        let mut cpu = Sm510::new();
        cpu.rom[0] = 0x4B; // LB $B
        cpu.step();
        assert_eq!(cpu.bl, 0xB);
    }

    #[test]
    fn test_add() {
        let mut cpu = Sm510::new();
        cpu.acc = 3;
        cpu.bl = 0;
        cpu.bm = 0;
        cpu.ram[0] = 5;
        cpu.rom[0] = 0x08; // ADD
        cpu.step();
        assert_eq!(cpu.acc, 8);
        assert!(!cpu.carry);

        // Overflow
        cpu.acc = 0xA;
        cpu.pc = 0;
        cpu.ram[0] = 0x8;
        cpu.step();
        assert_eq!(cpu.acc, 2); // (0xA + 0x8) & 0xF = 2
        assert!(cpu.carry);
    }

    #[test]
    fn test_add11_with_carry() {
        let mut cpu = Sm510::new();
        cpu.acc = 3;
        cpu.carry = true;
        cpu.bl = 0;
        cpu.bm = 0;
        cpu.ram[0] = 4;
        cpu.rom[0] = 0x09; // ADD11
        cpu.step();
        assert_eq!(cpu.acc, 8); // 3 + 4 + 1 = 8
        assert!(!cpu.carry);
    }

    #[test]
    fn test_coma() {
        let mut cpu = Sm510::new();
        cpu.acc = 0x5;
        cpu.rom[0] = 0x0A; // COMA
        cpu.step();
        assert_eq!(cpu.acc, 0xA); // ~0x5 & 0xF = 0xA
    }

    #[test]
    fn test_exbla() {
        let mut cpu = Sm510::new();
        cpu.acc = 0x3;
        cpu.bl = 0x7;
        cpu.rom[0] = 0x0B; // EXBLA
        cpu.step();
        assert_eq!(cpu.acc, 0x7);
        assert_eq!(cpu.bl, 0x3);
    }

    #[test]
    fn test_rm_sm() {
        let mut cpu = Sm510::new();
        cpu.bl = 0;
        cpu.bm = 0;
        cpu.ram[0] = 0xF;

        // RM 1 - reset bit 1
        cpu.rom[0] = 0x05;
        cpu.step();
        assert_eq!(cpu.ram[0], 0xD); // 1111 -> 1101

        // SM 1 - set bit 1 back
        cpu.pc = 0;
        cpu.rom[0] = 0x0D;
        cpu.step();
        assert_eq!(cpu.ram[0], 0xF);
    }

    #[test]
    fn test_exc() {
        let mut cpu = Sm510::new();
        cpu.acc = 0x3;
        cpu.bl = 0;
        cpu.bm = 0;
        cpu.ram[0] = 0x9;
        cpu.rom[0] = 0x10; // EXC
        cpu.step();
        assert_eq!(cpu.acc, 0x9);
        assert_eq!(cpu.ram[0], 0x3);
    }

    #[test]
    fn test_exci_with_overflow() {
        let mut cpu = Sm510::new();
        cpu.acc = 0x5;
        cpu.bl = 0xF; // Will overflow to 0
        cpu.bm = 0;
        cpu.ram[0x0F] = 0xA;
        cpu.rom[0] = 0x14; // EXCI
        cpu.step();
        assert_eq!(cpu.acc, 0xA);
        assert_eq!(cpu.ram[0x0F], 0x5);
        assert_eq!(cpu.bl, 0); // Overflowed
        assert!(cpu.skip); // Skip on overflow
    }

    #[test]
    fn test_excd_with_underflow() {
        let mut cpu = Sm510::new();
        cpu.acc = 0x3;
        cpu.bl = 0; // Will underflow to 0xF
        cpu.bm = 0;
        cpu.ram[0] = 0x7;
        cpu.rom[0] = 0x1C; // EXCD
        cpu.step();
        assert_eq!(cpu.acc, 0x7);
        assert_eq!(cpu.ram[0], 0x3);
        assert_eq!(cpu.bl, 0xF); // Underflowed
        assert!(cpu.skip); // Skip on underflow
    }

    #[test]
    fn test_tc() {
        let mut cpu = Sm510::new();
        cpu.carry = true;
        cpu.rom[0] = 0x52; // TC
        cpu.step();
        assert!(cpu.skip);

        cpu.carry = false;
        cpu.pc = 0;
        cpu.skip = false;
        cpu.step();
        assert!(!cpu.skip);
    }

    #[test]
    fn test_ta0() {
        let mut cpu = Sm510::new();
        cpu.acc = 0;
        cpu.rom[0] = 0x5A; // TA0
        cpu.step();
        assert!(cpu.skip);

        cpu.acc = 5;
        cpu.pc = 0;
        cpu.skip = false;
        cpu.step();
        assert!(!cpu.skip);
    }

    #[test]
    fn test_rc_sc() {
        let mut cpu = Sm510::new();
        cpu.rom[0] = 0x67; // SC
        cpu.rom[1] = 0x66; // RC
        cpu.step();
        assert!(cpu.carry);
        cpu.step();
        assert!(!cpu.carry);
    }

    #[test]
    fn test_incb() {
        let mut cpu = Sm510::new();
        cpu.bl = 5;
        cpu.rom[0] = 0x64; // INCB
        cpu.step();
        assert_eq!(cpu.bl, 6);
        assert!(!cpu.skip);

        // Test overflow
        cpu.bl = 0xF;
        cpu.pc = 0;
        cpu.step();
        assert_eq!(cpu.bl, 0);
        assert!(cpu.skip);
    }

    #[test]
    fn test_t_branch() {
        let mut cpu = Sm510::new();
        cpu.pc = 0x40; // Page 1 (0x40-0x7F)
        cpu.rom[0x40] = 0x8A; // T $0A → branch to page offset 0x0A
        cpu.step();
        assert_eq!(cpu.pc, 0x4A); // Same page, offset 0x0A
    }

    #[test]
    fn test_tl_branch() {
        let mut cpu = Sm510::new();
        cpu.rom[0] = 0x72; // TL page 2
        cpu.rom[1] = 0x15; // offset 0x15
        cpu.step();
        // Target = (2 << 6) | (0x15 & 0x3F) = 0x80 | 0x15 = 0x95
        assert_eq!(cpu.pc, 0x95);
    }

    #[test]
    fn test_tml_and_rtn() {
        let mut cpu = Sm510::new();
        // TML from address 0, call to page 1 offset 0x10
        cpu.rom[0] = 0x7C; // TML page 0
        cpu.rom[1] = 0x10; // offset 0x10
                           // At address 0x10: RTN0
        cpu.rom[0x10] = 0x6A; // RTN0

        cpu.step(); // Execute TML
        assert_eq!(cpu.pc, 0x10); // Jumped to 0x10
        assert_eq!(cpu.stack, 0x02); // Return address (after 2-byte TML)

        cpu.step(); // Execute RTN0
        assert_eq!(cpu.pc, 0x02); // Returned
    }

    #[test]
    fn test_lbl() {
        let mut cpu = Sm510::new();
        cpu.sbm = true;
        cpu.rom[0] = 0x5F; // LBL
        cpu.rom[1] = 0xA2; // BL=0xA, BM=0x2
        cpu.step();
        assert_eq!(cpu.bl, 0xA);
        assert_eq!(cpu.bm, 0x2);
        assert!(!cpu.sbm); // LBL clears SBM
    }

    #[test]
    fn test_cend() {
        let mut cpu = Sm510::new();
        cpu.rom[0] = 0x5D; // CEND
        cpu.step();
        assert!(cpu.halted);

        // CPU stays halted
        cpu.step();
        assert!(cpu.halted);

        // Wake up on K input
        cpu.controller_state = 0x01;
        cpu.update_input();
        cpu.output_s = 0x01; // Need S to be active for K to be non-zero
        cpu.update_input();
        cpu.input_k = 1; // Force wake
        cpu.step();
        assert!(!cpu.halted);
    }

    #[test]
    fn test_melody_control() {
        let mut cpu = Sm510::new();
        cpu.rom[0] = 0x6D; // SME
        cpu.step();
        assert!(cpu.melody_enabled);
        assert!(cpu.buzzer_active);

        cpu.rom[1] = 0x6F; // TME
        cpu.step();
        assert!(cpu.skip); // melody is enabled

        cpu.skip = false;
        cpu.rom[2] = 0x6E; // RME
        cpu.step();
        assert!(!cpu.melody_enabled);
        assert!(!cpu.buzzer_active);
    }

    #[test]
    fn test_tmi() {
        let mut cpu = Sm510::new();
        cpu.bl = 0;
        cpu.bm = 0;
        cpu.ram[0] = 0b1010; // bits 1 and 3 set

        cpu.rom[0] = 0x54; // TMI 0 → test bit 0
        cpu.step();
        assert!(!cpu.skip); // bit 0 is 0

        cpu.pc = 0;
        cpu.rom[0] = 0x55; // TMI 1 → test bit 1
        cpu.step();
        assert!(cpu.skip); // bit 1 is 1
    }

    #[test]
    fn test_tam() {
        let mut cpu = Sm510::new();
        cpu.bl = 0;
        cpu.bm = 0;
        cpu.acc = 0b0010;
        cpu.ram[0] = 0b1010;

        cpu.rom[0] = 0x53; // TAM
        cpu.step();
        assert!(cpu.skip); // 0010 & 1010 = 0010 != 0

        cpu.acc = 0b0100;
        cpu.pc = 0;
        cpu.skip = false;
        cpu.step();
        assert!(!cpu.skip); // 0100 & 1010 = 0000 == 0
    }

    #[test]
    fn test_ws_updates_input() {
        let mut cpu = Sm510::new();
        cpu.controller_state = 0x0005; // bits 0 and 2 set (Left + Up in row 0)
        cpu.rom[0] = 0x21; // LAX 1 (ACC = 1)
        cpu.rom[1] = 0x63; // WS (S = ACC = 1, selects row 0)
        cpu.rom[2] = 0x50; // KTA (read K input)
        cpu.step(); // LAX 1
        cpu.step(); // WS
        cpu.step(); // KTA
        assert_eq!(cpu.acc, 0x5); // K = directional row: bits 0 and 2
    }

    #[test]
    fn test_disassemble() {
        let rom = vec![0x00, 0x25, 0x72, 0x15, 0x6A, 0x80];

        let (m, l) = Sm510::disassemble(&rom, 0);
        assert_eq!(m, "SKIP");
        assert_eq!(l, 1);

        let (m, l) = Sm510::disassemble(&rom, 1);
        assert_eq!(m, "LAX $5");
        assert_eq!(l, 1);

        let (m, l) = Sm510::disassemble(&rom, 2);
        assert_eq!(m, "TL $095");
        assert_eq!(l, 2);

        let (m, l) = Sm510::disassemble(&rom, 4);
        assert_eq!(m, "RTN0");
        assert_eq!(l, 1);

        let (m, l) = Sm510::disassemble(&rom, 5);
        assert_eq!(m, "T $00");
        assert_eq!(l, 1);
    }

    #[test]
    fn test_pc_wraps_within_page() {
        let mut cpu = Sm510::new();
        cpu.pc = 0x3F; // Last byte of page 0
        cpu.increment_pc();
        assert_eq!(cpu.pc, 0x00); // Wraps to start of page 0, not page 1
    }

    #[test]
    fn test_sbm_ram_access() {
        let mut cpu = Sm510::new();
        cpu.bl = 0;
        cpu.bm = 0;
        cpu.sbm = false;

        // Without SBM: address = (0 << 4) | 0 = 0
        cpu.ram_write(0x5);
        assert_eq!(cpu.ram[0], 0x5);

        // With SBM: address = (4 << 4) | 0 = 64
        cpu.sbm = true;
        cpu.ram_write(0xA);
        assert_eq!(cpu.ram[64], 0xA);
        assert_eq!(cpu.ram[0], 0x5); // Original unchanged
    }
}
