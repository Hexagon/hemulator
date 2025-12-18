# TODO

## Completed
- [x] Implement minimal NES PPUSTATUS vblank bit and wire bus reads.
- [x] Add missing CPU opcodes needed for common ROM init loops (LDX/LDY, STX/STY, transfers, flag ops).
- [x] Add minimal NMI + RTI support and a per-frame vblank pulse.
- [x] Prevent writes to CHR-ROM from corrupting pattern tables.
- [x] Render background using attribute-table palettes (basic color support).
- [x] Honor PPUCTRL VRAM increment (1 vs 32) and base nametable select for rendering.
- [x] Render at end-of-visible (pre-VBlank) to avoid sampling while PPUMASK is toggled during NMI (fixes gray/black alternating frames).
- [x] Add mapper 1 (MMC1) for NES Tetris/SxROM carts.
- [x] Add mapper 2 (UxROM) for NES games like Mega Man.
- [x] Add mapper 4 (MMC3) for NES games like Super Mario Bros. 3.
- [x] Refactor mappers into separate modules with comprehensive unit tests.
- [x] Document supported mappers in README.
- [x] Add F12 reset key to GUI.
- [x] Create CI workflow for automated testing across platforms.
- [x] Clean up code warnings and improve code quality.
- [x] Implement mapper 3 (CNROM) and mapper 7 (AxROM) for broader game compatibility.

## In Progress / Future Work
- [ ] Reduce/disable debug logging and CLI dumps for normal runs (partially done with --keep-logs flag).
- [ ] Improve PPU register coverage (scroll + nametable switching, PPUDATA reads) for more accurate visuals.
- [ ] Add sprite rendering improvements (OAM evaluation + sprite 0 hit).
- [ ] Verify Tetris (Mapper 1) graphics rendering - requires testing with actual ROM.
- [ ] Verify SMB3 (Mapper 4) startup and IRQ timing - requires testing with actual ROM.
- [ ] Add configuration interface for resolution and other settings.
- [ ] Implement additional common mappers (9, 10, 11).
- [ ] Improve PPU timing accuracy for better game compatibility.
- [ ] Add save state UI management.
- [ ] Add audio configuration options.
