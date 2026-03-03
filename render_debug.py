"""
Manually render GBA BG0 from a debug dump to verify PPU correctness.
Reads VRAM, palette, and IO registers from the dump file.
"""
import re
import struct
from PIL import Image

SCREEN_W = 240
SCREEN_H = 160

def parse_dump(path):
    """Parse hex dump regions from debug dump file."""
    regions = {}
    current_addr = None
    current_data = bytearray()
    
    with open(path, 'r', encoding='utf-8', errors='ignore') as f:
        for line in f:
            # Match hex dump lines: "AAAAAAA: XX XX XX ... |...|"
            m = re.match(r'^([0-9A-Fa-f]+):\s+((?:[0-9A-Fa-f]{2}\s+)+)', line)
            if m:
                addr = int(m.group(1), 16)
                hex_bytes = bytes.fromhex(m.group(2).replace(' ', ''))
                
                if current_addr is None:
                    current_addr = addr
                    current_data = bytearray(hex_bytes)
                elif addr == current_addr + len(current_data):
                    current_data.extend(hex_bytes)
                else:
                    if current_addr is not None:
                        regions[current_addr] = bytes(current_data)
                    current_addr = addr
                    current_data = bytearray(hex_bytes)
            else:
                if current_addr is not None and len(current_data) > 0:
                    regions[current_addr] = bytes(current_data)
                    current_addr = None
                    current_data = bytearray()
    
    if current_addr is not None:
        regions[current_addr] = bytes(current_data)
    
    return regions

def get_region_data(regions, target_addr, size):
    """Get data from a specific address range across regions."""
    data = bytearray(size)
    for base, region_data in regions.items():
        if base <= target_addr < base + len(region_data):
            start_offset = target_addr - base
            available = min(size, len(region_data) - start_offset)
            data[:available] = region_data[start_offset:start_offset + available]
            return bytes(data)
    return bytes(data)

def gba_to_rgb(color16):
    """Convert GBA 15-bit color to RGB tuple."""
    r = (color16 & 0x1F)
    g = ((color16 >> 5) & 0x1F)
    b = ((color16 >> 10) & 0x1F)
    r8 = (r << 3) | (r >> 2)
    g8 = (g << 3) | (g >> 2)
    b8 = (b << 3) | (b >> 2)
    return (r8, g8, b8)

def main():
    dump_path = 'tetris_dump4.txt'
    regions = parse_dump(dump_path)
    
    print(f"Parsed {len(regions)} memory regions:")
    for addr, data in sorted(regions.items()):
        print(f"  0x{addr:08X}: {len(data)} bytes")
    
    # Get memory regions
    io = get_region_data(regions, 0x04000000, 0x400)
    palette = get_region_data(regions, 0x05000000, 0x400)
    vram = get_region_data(regions, 0x06000000, 0x18000)
    oam = get_region_data(regions, 0x07000000, 0x400)
    
    # Parse DISPCNT
    dispcnt = struct.unpack_from('<H', io, 0)[0]
    bg_mode = dispcnt & 7
    print(f"\nDISPCNT = 0x{dispcnt:04X}")
    print(f"  BG Mode: {bg_mode}")
    print(f"  OBJ 1D mapping: {bool(dispcnt & 0x40)}")
    print(f"  Forced blank: {bool(dispcnt & 0x80)}")
    for i in range(4):
        enabled = bool(dispcnt & (1 << (8 + i)))
        print(f"  BG{i}: {'enabled' if enabled else 'disabled'}")
    print(f"  OBJ: {'enabled' if dispcnt & 0x1000 else 'disabled'}")
    for w in ['WIN0', 'WIN1', 'OBJ_WIN']:
        bit = 13 + ['WIN0', 'WIN1', 'OBJ_WIN'].index(w)
        print(f"  {w}: {'enabled' if dispcnt & (1 << bit) else 'disabled'}")
    
    # Parse BG0CNT
    bg0cnt = struct.unpack_from('<H', io, 0x08)[0]
    priority = bg0cnt & 3
    tile_base = ((bg0cnt >> 2) & 3) * 0x4000
    is_8bpp = bool(bg0cnt & 0x80)
    map_base = ((bg0cnt >> 8) & 0x1F) * 0x800
    size_bits = (bg0cnt >> 14) & 3
    
    TEXT_BG_SIZES = [(32, 32), (64, 32), (32, 64), (64, 64)]
    map_w, map_h = TEXT_BG_SIZES[size_bits]
    
    print(f"\nBG0CNT = 0x{bg0cnt:04X}")
    print(f"  Priority: {priority}")
    print(f"  Tile base: 0x{tile_base:04X}")
    print(f"  8bpp: {is_8bpp}")
    print(f"  Map base: 0x{map_base:04X}")
    print(f"  Size: {map_w}x{map_h} tiles ({map_w*8}x{map_h*8} pixels)")
    
    # Parse scroll
    scroll_x = struct.unpack_from('<H', io, 0x10)[0] & 0x1FF
    scroll_y = struct.unpack_from('<H', io, 0x12)[0] & 0x1FF
    print(f"  Scroll: ({scroll_x}, {scroll_y})")
    
    # Check some tilemap entries
    print(f"\nFirst 16 tilemap entries at map_base 0x{map_base:04X}:")
    for i in range(16):
        entry_addr = map_base + i * 2
        entry = struct.unpack_from('<H', vram, entry_addr)[0]
        tile_id = entry & 0x3FF
        hflip = bool(entry & 0x400)
        vflip = bool(entry & 0x800)
        pal = (entry >> 12) & 0xF
        print(f"  [{i:2d}] 0x{entry:04X}: tile={tile_id:3d} hflip={hflip} vflip={vflip} pal={pal}")
    
    # Check non-zero entries in entire tilemap
    total_entries = map_w * map_h
    nonzero = 0
    for i in range(total_entries):
        # Account for screen blocks
        tile_col = i % map_w
        tile_row = i // map_w
        screen_block = 0
        if map_w == 64 and map_h == 64:
            screen_block = (tile_row // 32) * 2 + tile_col // 32
        elif map_w == 64:
            screen_block = tile_col // 32
        elif map_h == 64:
            screen_block = tile_row // 32
        local_col = tile_col & 31
        local_row = tile_row & 31
        entry_addr = map_base + screen_block * 0x800 + (local_row * 32 + local_col) * 2
        if entry_addr + 1 < len(vram):
            entry = struct.unpack_from('<H', vram, entry_addr)[0]
            if entry != 0:
                nonzero += 1
    print(f"\nNon-zero tilemap entries: {nonzero}/{total_entries}")
    
    # Print first 8 palette entries
    print(f"\nFirst 16 BG palette entries:")
    for i in range(16):
        color = struct.unpack_from('<H', palette, i * 2)[0]
        r, g, b = gba_to_rgb(color)
        print(f"  [{i:2d}] 0x{color:04X} -> RGB({r:3d},{g:3d},{b:3d})")
    
    # Render BG0 manually
    img = Image.new('RGB', (SCREEN_W, SCREEN_H), (0, 0, 0))
    backdrop_color = struct.unpack_from('<H', palette, 0)[0]
    
    for y in range(SCREEN_H):
        for x in range(SCREEN_W):
            screen_x = (x + scroll_x) & (map_w * 8 - 1)
            screen_y = (y + scroll_y) & (map_h * 8 - 1)
            tile_col = screen_x // 8
            tile_row = screen_y // 8
            fine_x = screen_x & 7
            fine_y = screen_y & 7
            
            # Screen block
            screen_block = 0
            if map_w == 64 and map_h == 64:
                screen_block = (tile_row // 32) * 2 + tile_col // 32
            elif map_w == 64:
                screen_block = tile_col // 32
            elif map_h == 64:
                screen_block = tile_row // 32
            local_col = tile_col & 31
            local_row = tile_row & 31
            
            entry_addr = map_base + screen_block * 0x800 + (local_row * 32 + local_col) * 2
            if entry_addr + 1 >= len(vram):
                continue
            
            entry = struct.unpack_from('<H', vram, entry_addr)[0]
            tile_id = entry & 0x3FF
            hflip = bool(entry & 0x400)
            vflip = bool(entry & 0x800)
            pal_bank = (entry >> 12) & 0xF
            
            px = (7 - fine_x) if hflip else fine_x
            py = (7 - fine_y) if vflip else fine_y
            
            if is_8bpp:
                tile_addr = tile_base + tile_id * 64 + py * 8 + px
                if tile_addr < len(vram):
                    color_idx = vram[tile_addr]
                else:
                    color_idx = 0
            else:
                tile_addr = tile_base + tile_id * 32 + py * 4 + px // 2
                if tile_addr < len(vram):
                    byte = vram[tile_addr]
                    color_idx = (byte & 0x0F) if (px & 1) == 0 else (byte >> 4)
                else:
                    color_idx = 0
            
            if color_idx == 0:
                # Transparent -> backdrop
                color16 = backdrop_color
            else:
                if is_8bpp:
                    pal_idx = color_idx
                else:
                    pal_idx = pal_bank * 16 + color_idx
                color16 = struct.unpack_from('<H', palette, pal_idx * 2)[0]
            
            img.putpixel((x, y), gba_to_rgb(color16))
    
    img.save('manual_render_bg0.png')
    print(f"\nManual BG0 render saved to manual_render_bg0.png")
    
    # Also render OBJ layer
    # Check how many sprites are visible
    visible_sprites = 0
    for i in range(128):
        attr0 = struct.unpack_from('<H', oam, i * 8)[0]
        mode = attr0 & 0x0300
        if mode != 0x0200:  # Not hidden
            y = attr0 & 0xFF
            attr1 = struct.unpack_from('<H', oam, i * 8 + 2)[0]
            x = attr1 & 0x1FF
            if x >= 256: x -= 512
            attr2 = struct.unpack_from('<H', oam, i * 8 + 4)[0]
            tile = attr2 & 0x3FF
            if tile != 0 or y != 0 or x != 0:
                visible_sprites += 1
                if visible_sprites <= 10:
                    shape = (attr0 >> 14) & 3
                    size = (attr1 >> 14) & 3
                    pri = (attr2 >> 10) & 3
                    pal = (attr2 >> 12) & 0xF
                    is_8bpp_obj = bool(attr0 & 0x2000)
                    print(f"  OBJ[{i:3d}]: x={x:4d} y={y:3d} tile={tile:3d} shape={shape} size={size} pri={pri} pal={pal} 8bpp={is_8bpp_obj}")
    
    print(f"\nVisible sprites: {visible_sprites}")

if __name__ == '__main__':
    main()
