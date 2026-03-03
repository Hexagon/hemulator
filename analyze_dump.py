"""Analyze a debug dump file."""
import re, sys

fname = sys.argv[1] if len(sys.argv) > 1 else 'tetris_50m_fix.txt'
with open(fname, 'r', errors='ignore') as f:
    content = f.read()

io = bytearray(0x400)
vram_nonzero = 0
oam_nonzero = 0
pal_data = bytearray(0x400)

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
                pal_data[offset + i] = b
    elif 0x6000000 <= addr < 0x6018000:
        for b in data:
            if b != 0:
                vram_nonzero += 1
    elif 0x7000000 <= addr < 0x7000400:
        for b in data:
            if b != 0:
                oam_nonzero += 1

dispcnt = io[0] | (io[1] << 8)
bg0cnt = io[8] | (io[9] << 8)
bg_mode = dispcnt & 7
layers = []
for i in range(4):
    if dispcnt & (1 << (8+i)):
        layers.append(f"BG{i}")
if dispcnt & (1 << 12):
    layers.append("OBJ")
forced = bool(dispcnt & (1 << 7))

bg_pal_nonzero = sum(1 for i in range(0, 0x200, 2) if pal_data[i] | pal_data[i+1])
obj_pal_nonzero = sum(1 for i in range(0x200, 0x400, 2) if pal_data[i] | pal_data[i+1])

hofs0 = (io[0x10] | (io[0x11] << 8)) & 0x1FF
vofs0 = (io[0x12] | (io[0x13] << 8)) & 0x1FF

print(f"DISPCNT=0x{dispcnt:04X} Mode={bg_mode} Layers={'+'.join(layers)} Forced={forced}")
print(f"BG0CNT=0x{bg0cnt:04X}")
print(f"VRAM non-zero: {vram_nonzero}")
print(f"OAM non-zero: {oam_nonzero}")
print(f"Palette: BG={bg_pal_nonzero}/256, OBJ={obj_pal_nonzero}/256")
print(f"BG0 scroll: H={hofs0} V={vofs0}")
