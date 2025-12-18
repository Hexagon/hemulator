# TODO

- [x] Implement minimal NES PPUSTATUS vblank bit and wire bus reads.
- [x] Add missing CPU opcodes needed for common ROM init loops (LDX/LDY, STX/STY, transfers, flag ops).
- [x] Add minimal NMI + RTI support and a per-frame vblank pulse.
- [x] Prevent writes to CHR-ROM from corrupting pattern tables.

- [x] Render background using attribute-table palettes (basic color support).
- [x] Honor PPUCTRL VRAM increment (1 vs 32) and base nametable select for rendering.

- [x] Render at end-of-visible (pre-VBlank) to avoid sampling while PPUMASK is toggled during NMI (fixes gray/black alternating frames).

- [ ] Reduce/disable debug logging and CLI dumps for normal runs.
- [ ] Improve PPU register coverage (scroll + nametable switching, PPUDATA reads) for more accurate visuals.
- [ ] Add sprite rendering (OAM evaluation + sprite 0 hit).
