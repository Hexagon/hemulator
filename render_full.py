"""Full frame renderer from debug dump - renders both BG0 and OBJ layers."""
import re, struct, sys
from PIL import Image

fname = sys.argv[1] if len(sys.argv) > 1 else 'tetris_200m_fix.txt'
with open(fname, 'r', errors='ignore') as f:
    content = f.read()

# Parse all memory regions
io = bytearray(0x400)
palette = bytearray(0x400) 
vram = bytearray(0x18000)
oam = bytearray(0x400)

for m in re.finditer(r'^([0-9A-Fa-f]{7,8}):\s+((?:[0-9A-Fa-f]{2}\s+)+)', content, re.M):
    addr = int(m.group(1), 16)
    data = bytes.fromhex(m.group(2).replace(' ', ''))
    if 0x4000000 <= addr < 0x4000400:
        off = addr - 0x4000000
        for i, b in enumerate(data):
            if off + i < 0x400: io[off + i] = b
    elif 0x5000000 <= addr < 0x5000400:
        off = addr - 0x5000000
        for i, b in enumerate(data):
            if off + i < 0x400: palette[off + i] = b
    elif 0x6000000 <= addr < 0x6018000:
        off = addr - 0x6000000
        for i, b in enumerate(data):
            if off + i < 0x18000: vram[off + i] = b
    elif 0x7000000 <= addr < 0x7000400:
        off = addr - 0x7000000
        for i, b in enumerate(data):
            if off + i < 0x400: oam[off + i] = b

def gba_to_rgb(c):
    r = (c & 0x1F)
    g = ((c >> 5) & 0x1F)
    b = ((c >> 10) & 0x1F)
    return ((r << 3) | (r >> 2), (g << 3) | (g >> 2), (b << 3) | (b >> 2))

def pal_lookup(idx):
    off = idx * 2
    if off + 1 < len(palette):
        return struct.unpack_from('<H', palette, off)[0]
    return 0

dispcnt = io[0] | (io[1] << 8)
bg_mode = dispcnt & 7
print(f"DISPCNT=0x{dispcnt:04X} Mode={bg_mode}")

OBJ_SIZES = [
    [(8,8),(16,16),(32,32),(64,64)],   # Square
    [(16,8),(32,8),(32,16),(64,32)],    # Horizontal
    [(8,16),(8,32),(16,32),(32,64)],    # Vertical
]

# Render full frame
W, H = 240, 160
frame = Image.new('RGB', (W, H), (0, 0, 0))
pixels = frame.load()

# BG0 rendering (Mode 0, text mode)
if dispcnt & (1 << 8):  # BG0 enabled
    bgcnt = io[8] | (io[9] << 8)
    priority = bgcnt & 3
    tile_base = ((bgcnt >> 2) & 3) * 0x4000
    is_8bpp = bool(bgcnt & 0x80)
    map_base = ((bgcnt >> 8) & 0x1F) * 0x800
    size_bits = (bgcnt >> 14) & 3
    sizes = [(32,32),(64,32),(32,64),(64,64)]
    map_w, map_h = sizes[size_bits]
    
    hofs = (io[0x10] | (io[0x11] << 8)) & 0x1FF
    vofs = (io[0x12] | (io[0x13] << 8)) & 0x1FF
    
    print(f"BG0: tile_base=0x{tile_base:X} map_base=0x{map_base:X} 4bpp={not is_8bpp} {map_w}x{map_h} scroll=({hofs},{vofs})")
    
    bg_layer = [[None]*W for _ in range(H)]
    
    for y in range(H):
        for x in range(W):
            sx = (x + hofs) & (map_w * 8 - 1)
            sy = (y + vofs) & (map_h * 8 - 1)
            tile_col = sx // 8
            tile_row = sy // 8
            fine_x = sx & 7
            fine_y = sy & 7
            
            # Screen block
            if map_w == 64 and map_h == 64:
                sb = (tile_row // 32) * 2 + tile_col // 32
            elif map_w == 64:
                sb = tile_col // 32
            elif map_h == 64:
                sb = tile_row // 32
            else:
                sb = 0
            
            lc = tile_col & 31
            lr = tile_row & 31
            
            entry_addr = map_base + sb * 0x800 + (lr * 32 + lc) * 2
            if entry_addr + 1 >= len(vram):
                continue
            entry = vram[entry_addr] | (vram[entry_addr + 1] << 8)
            tile_id = entry & 0x3FF
            hflip = bool(entry & (1 << 10))
            vflip = bool(entry & (1 << 11))
            pal_bank = (entry >> 12) & 0xF
            
            py = (7 - fine_y) if vflip else fine_y
            px = (7 - fine_x) if hflip else fine_x
            
            if is_8bpp:
                ta = tile_base + tile_id * 64 + py * 8 + px
                ci = vram[ta] if ta < len(vram) else 0
            else:
                ta = tile_base + tile_id * 32 + py * 4 + px // 2
                if ta < len(vram):
                    b = vram[ta]
                    ci = (b & 0xF) if (px & 1) == 0 else (b >> 4)
                else:
                    ci = 0
            
            if ci == 0:
                continue
            
            pi = (pal_bank * 16 + ci) if not is_8bpp else ci
            color = pal_lookup(pi)
            bg_layer[y][x] = (gba_to_rgb(color), priority)

# OBJ rendering
obj_1d = bool(dispcnt & (1 << 6))
if dispcnt & (1 << 12):  # OBJ enabled
    obj_layer = [[None]*W for _ in range(H)]
    
    sprite_count = 0
    visible_sprites = 0
    for idx in range(127, -1, -1):  # Reverse order (lower index = higher priority)
        off = idx * 8
        attr0 = oam[off] | (oam[off+1] << 8)
        attr1 = oam[off+2] | (oam[off+3] << 8)
        attr2 = oam[off+4] | (oam[off+5] << 8)
        
        mode = attr0 & 0x0300
        if mode == 0x0200:  # Hidden
            continue
        
        shape = (attr0 >> 14) & 3
        size = (attr1 >> 14) & 3
        if shape >= 3:
            continue
        
        ow, oh = OBJ_SIZES[shape][size]
        is_affine = mode in (0x0100, 0x0300)
        double = mode == 0x0300
        bw = ow * 2 if double else ow
        bh = oh * 2 if double else oh
        
        obj_y = attr0 & 0xFF
        obj_x = attr1 & 0x1FF
        if obj_x >= 256: obj_x -= 512
        if obj_y >= 160 and obj_y + bh > 256: obj_y -= 256
        
        tile_base_id = attr2 & 0x3FF
        pri = (attr2 >> 10) & 3
        pal_bank_obj = (attr2 >> 12) & 0xF
        is_8bpp_obj = bool(attr0 & (1 << 13))
        
        hflip = bool(attr1 & (1 << 12)) and not is_affine
        vflip = bool(attr1 & (1 << 13)) and not is_affine
        
        rendered = False
        for screen_y in range(H):
            ly = screen_y - obj_y
            if ly < 0 or ly >= bh:
                continue
            
            for lx_i in range(bw):
                sx = obj_x + lx_i
                if sx < 0 or sx >= W:
                    continue
                
                if is_affine:
                    # Skip affine sprites for now (complex)
                    continue
                
                tx = (ow - 1 - lx_i) if hflip else lx_i
                ty = (oh - 1 - ly) if vflip else ly
                
                tile_x = tx // 8
                tile_y = ty // 8
                fx = tx & 7
                fy = ty & 7
                
                if obj_1d:
                    tiles_per_row = ow // 8
                    tile_offset = tile_y * tiles_per_row + tile_x
                    if is_8bpp_obj:
                        tile_id = tile_base_id + tile_offset * 2
                    else:
                        tile_id = tile_base_id + tile_offset
                else:
                    if is_8bpp_obj:
                        tile_id = tile_base_id + tile_y * 32 + tile_x * 2
                    else:
                        tile_id = tile_base_id + tile_y * 32 + tile_x
                
                obj_vram_base = 0x10000
                if is_8bpp_obj:
                    ta = obj_vram_base + tile_id * 32 + fy * 8 + fx
                    ci = vram[ta] if ta < len(vram) else 0
                else:
                    ta = obj_vram_base + tile_id * 32 + fy * 4 + fx // 2
                    if ta < len(vram):
                        b = vram[ta]
                        ci = (b & 0xF) if (fx & 1) == 0 else (b >> 4)
                    else:
                        ci = 0
                
                if ci == 0:
                    continue
                
                pi = (pal_bank_obj * 16 + ci) if not is_8bpp_obj else ci
                color = pal_lookup(128 + pi)  # OBJ palette starts at entry 128 (offset 0x200)
                obj_layer[screen_y][sx] = (gba_to_rgb(color), pri)
                rendered = True
        
        if rendered:
            visible_sprites += 1
        sprite_count += 1
    
    print(f"Sprites processed: {sprite_count}, visible: {visible_sprites}")

# Composite: BG0 and OBJ, lower priority number wins
for y in range(H):
    for x in range(W):
        bg = bg_layer[y][x] if bg_layer[y][x] else None
        obj = obj_layer[y][x] if obj_layer[y][x] else None
        
        if bg and obj:
            if obj[1] <= bg[1]:
                pixels[x, y] = obj[0]
            else:
                pixels[x, y] = bg[0]
        elif bg:
            pixels[x, y] = bg[0]
        elif obj:
            pixels[x, y] = obj[0]

frame.save('manual_full_render.png')
print(f"Saved manual_full_render.png")

# Compare with emulator screenshot
import os, collections
screenshots = sorted([f for f in os.listdir('screenshots/gba') if f.endswith('.png')], 
                     key=lambda f: os.path.getmtime(f'screenshots/gba/{f}'))
if screenshots:
    emu_img = Image.open(f'screenshots/gba/{screenshots[-1]}')
    emu_pixels = emu_img.load()
    
    diffs = 0
    only_emu = 0
    only_manual = 0
    for y in range(H):
        for x in range(W):
            mp = pixels[x, y]
            ep = emu_pixels[x, y][:3]
            if mp != ep:
                diffs += 1
                if mp == (0,0,0) and ep != (0,0,0):
                    only_emu += 1
                elif mp != (0,0,0) and ep == (0,0,0):
                    only_manual += 1
    
    manual_nonblack = sum(1 for y in range(H) for x in range(W) if pixels[x,y] != (0,0,0))
    emu_nonblack = sum(1 for y in range(H) for x in range(W) if emu_pixels[x,y][:3] != (0,0,0))
    
    print(f"\nComparison with emulator:")
    print(f"  Manual non-black: {manual_nonblack}")
    print(f"  Emulator non-black: {emu_nonblack}")
    print(f"  Different pixels: {diffs}")
    print(f"  Only in emulator: {only_emu}")
    print(f"  Only in manual: {only_manual}")
