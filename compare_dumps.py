"""Compare IO registers between two debug dumps to see if the game is making progress."""
import re, struct, sys

def parse_dump(filename):
    with open(filename, 'r', errors='ignore') as f:
        content = f.read()
    
    io = bytearray(0x400)
    palette = bytearray(0x400)
    vram_nonzero = 0
    oam_nonzero = 0
    
    for m in re.finditer(r'^([0-9A-Fa-f]{7,8}):\s+((?:[0-9A-Fa-f]{2}\s+)+)', content, re.M):
        addr = int(m.group(1), 16)
        data = bytes.fromhex(m.group(2).replace(' ', ''))
        
        if 0x4000000 <= addr < 0x4000400:
            offset = addr - 0x4000000
            for i, b in enumerate(data):
                if offset + i < 0x400:
                    io[offset + i] = b
        elif 0x5000000 <= addr < 0x5000400:
            offset = addr - 0x5000000
            for i, b in enumerate(data):
                if offset + i < 0x400:
                    palette[offset + i] = b
        elif 0x6000000 <= addr < 0x6018000:
            for b in data:
                if b != 0:
                    vram_nonzero += 1
        elif 0x7000000 <= addr < 0x7000400:
            for b in data:
                if b != 0:
                    oam_nonzero += 1
    
    # Decode key registers
    dispcnt = io[0] | (io[1] << 8)
    bg0cnt = io[8] | (io[9] << 8)
    bg1cnt = io[0xA] | (io[0xB] << 8)
    bg2cnt = io[0xC] | (io[0xD] << 8)
    bg3cnt = io[0xE] | (io[0xF] << 8)
    ime_val = io[0x208] | (io[0x209] << 8) if len(io) > 0x209 else 0
    ie_val = io[0x200] | (io[0x201] << 8)
    if_val = io[0x202] | (io[0x203] << 8)
    
    bg_mode = dispcnt & 7
    bg_enables = []
    for i in range(4):
        if dispcnt & (1 << (8+i)):
            bg_enables.append(f"BG{i}")
    if dispcnt & (1 << 12):
        bg_enables.append("OBJ")
    
    return {
        'dispcnt': dispcnt,
        'bg0cnt': bg0cnt, 'bg1cnt': bg1cnt, 'bg2cnt': bg2cnt, 'bg3cnt': bg3cnt,
        'ime': ime_val, 'ie': ie_val, 'if': if_val,
        'bg_mode': bg_mode,
        'bg_enables': '+'.join(bg_enables) if bg_enables else 'NONE',
        'vram_nonzero': vram_nonzero,
        'oam_nonzero': oam_nonzero,
        'forced_blank': bool(dispcnt & (1 << 7)),
        'io': io,
        'palette': palette,
    }

for fname in ['tetris_dump_5m.txt', 'tetris_dump4.txt']:
    d = parse_dump(fname)
    label = fname.replace('tetris_dump_', '').replace('.txt', '')
    print(f"=== {label} ===")
    print(f"  DISPCNT=0x{d['dispcnt']:04X} Mode={d['bg_mode']} Layers={d['bg_enables']} Forced={d['forced_blank']}")
    print(f"  BG0CNT=0x{d['bg0cnt']:04X} BG1CNT=0x{d['bg1cnt']:04X} BG2CNT=0x{d['bg2cnt']:04X} BG3CNT=0x{d['bg3cnt']:04X}")
    print(f"  IME=0x{d['ime']:04X} IE=0x{d['ie']:04X} IF=0x{d['if']:04X}")
    print(f"  VRAM non-zero bytes: {d['vram_nonzero']}")
    print(f"  OAM non-zero bytes: {d['oam_nonzero']}")
    
    # Count non-zero palette entries  
    bg_pal_nonzero = sum(1 for i in range(0, 0x200, 2) if d['palette'][i] | d['palette'][i+1])
    obj_pal_nonzero = sum(1 for i in range(0x200, 0x400, 2) if d['palette'][i] | d['palette'][i+1])
    print(f"  Palette: BG={bg_pal_nonzero}/256 entries, OBJ={obj_pal_nonzero}/256 entries")
    
    # Scroll/Window regs 
    hofs0 = d['io'][0x10] | (d['io'][0x11] << 8)
    vofs0 = d['io'][0x12] | (d['io'][0x13] << 8)
    print(f"  BG0 scroll: H={hofs0 & 0x1FF} V={vofs0 & 0x1FF}")
    
    # BLDCNT
    bldcnt = d['io'][0x50] | (d['io'][0x51] << 8)
    blend_mode = (bldcnt >> 6) & 3
    print(f"  BLDCNT=0x{bldcnt:04X} blend_mode={blend_mode}")
    
    # Window regs
    win0h = d['io'][0x40] | (d['io'][0x41] << 8)
    win0v = d['io'][0x44] | (d['io'][0x45] << 8)
    win1h = d['io'][0x42] | (d['io'][0x43] << 8)
    win1v = d['io'][0x46] | (d['io'][0x47] << 8)
    winin = d['io'][0x48] | (d['io'][0x49] << 8)
    winout = d['io'][0x4A] | (d['io'][0x4B] << 8)
    print(f"  WIN0H=0x{win0h:04X} WIN0V=0x{win0v:04X} WIN1H=0x{win1h:04X} WIN1V=0x{win1v:04X}")
    print(f"  WININ=0x{winin:04X} WINOUT=0x{winout:04X}")
    print()
