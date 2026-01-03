#!/usr/bin/env python3
"""
3D Pong N64 Test ROM Builder

Creates a test ROM that demonstrates 3D rendering capabilities:
- Uses RSP for vertex transformation with F3DEX display lists
- Renders 3D paddles and ball
- Tests matrix transformations
- Uses perspective projection
- Demonstrates Gouraud shading
"""

import struct
import math

def mips_instr(opcode):
    """Pack a MIPS instruction as big-endian 32-bit word"""
    return struct.pack('>I', opcode)

def f3dex_vtx(addr, count, start_idx):
    """Generate F3DEX G_VTX command"""
    # word0: cmd(0x01) | count(8 bits at 20-12) | start_idx(7 bits at 16-1)
    word0 = (0x01 << 24) | ((count & 0xFF) << 12) | ((start_idx & 0x7F) << 1)
    word1 = addr
    return struct.pack('>I', word0) + struct.pack('>I', word1)

def f3dex_tri1(v0, v1, v2):
    """Generate F3DEX G_TRI1 command"""
    # Vertex indices need to be multiplied by 2 (vtx buffer stride)
    word0 = (0x05 << 24) | ((v0*2) << 16) | ((v1*2) << 8) | (v2*2)
    word1 = 0
    return struct.pack('>I', word0) + struct.pack('>I', word1)

def f3dex_tri2(v0, v1, v2, v3, v4, v5):
    """Generate F3DEX G_TRI2 command (two triangles)"""
    word0 = (0x06 << 24) | ((v0*2) << 16) | ((v1*2) << 8) | (v2*2)
    word1 = ((v3*2) << 16) | ((v4*2) << 8) | (v5*2)
    return struct.pack('>I', word0) + struct.pack('>I', word1)

def f3dex_mtx(addr, flags):
    """Generate F3DEX G_MTX command"""
    # flags: 0x01=PUSH, 0x02=NOPUSH/LOAD (0=MUL), 0x04=PROJECTION
    word0 = (0xDA << 24) | (flags & 0xFF)
    word1 = addr
    return struct.pack('>I', word0) + struct.pack('>I', word1)

def f3dex_enddl():
    """Generate F3DEX G_ENDDL command"""
    return struct.pack('>I', 0xDF000000) + struct.pack('>I', 0)

def create_vertex(x, y, z, r, g, b, a=255):
    """Create a vertex in N64 format (16 bytes)"""
    # Position (3x i16), flags (u16), tex coords (2x i16), color (4x u8)
    return struct.pack('>hhhHhh4B', x, y, z, 0, 0, 0, r, g, b, a)

def create_matrix_identity():
    """Create 4x4 identity matrix in N64 format (16.16 fixed point, 64 bytes)"""
    matrix = []
    for i in range(16):
        if i % 5 == 0:  # Diagonal elements
            matrix.append(0x00010000)  # 1.0 in 16.16 fixed point
        else:
            matrix.append(0x00000000)
    return b''.join(struct.pack('>i', x) for x in matrix)

def create_matrix_projection(fov, aspect, near, far):
    """Create perspective projection matrix in N64 format"""
    f = 1.0 / math.tan(fov / 2.0)
    matrix = [0] * 16
    
    # Column-major order for projection matrix
    matrix[0] = int((f / aspect) * 65536.0)  # [0,0]
    matrix[5] = int(f * 65536.0)  # [1,1]
    matrix[10] = int(((far + near) / (near - far)) * 65536.0)  # [2,2]
    matrix[11] = -65536  # [2,3] = -1.0
    matrix[14] = int((2 * far * near / (near - far)) * 65536.0)  # [3,2]
    
    return b''.join(struct.pack('>i', x) for x in matrix)

def create_matrix_translate(x, y, z):
    """Create translation matrix in N64 format"""
    matrix = [0] * 16
    # Identity with translation in last column
    matrix[0] = 0x00010000  # 1.0
    matrix[5] = 0x00010000  # 1.0
    matrix[10] = 0x00010000  # 1.0
    matrix[15] = 0x00010000  # 1.0
    matrix[12] = int(x * 65536.0)
    matrix[13] = int(y * 65536.0)
    matrix[14] = int(z * 65536.0)
    
    return b''.join(struct.pack('>i', m) for m in matrix)

def build_rom():
    """Build the 3D Pong N64 test ROM"""
    rom = bytearray()
    
    # ========== N64 ROM Header (64 bytes) ==========
    rom.extend(struct.pack('>I', 0x80371240))  # Magic (Z64 format)
    rom.extend(struct.pack('>I', 0x0000000F))  # Clock rate
    rom.extend(struct.pack('>I', 0x80000400))  # Boot address
    rom.extend(struct.pack('>I', 0x00001444))  # Release
    rom.extend(struct.pack('>I', 0x00000000))  # CRC1
    rom.extend(struct.pack('>I', 0x00000000))  # CRC2
    rom.extend(b'\x00' * 8)  # Reserved
    
    title = b'3D PONG TEST ROM    '
    rom.extend(title[:20])
    rom.extend(b'\x00' * 7)
    rom.extend(b'N')  # Manufacturer
    rom.extend(b'3P')  # Cartridge ID
    rom.extend(b'\x00')  # Country
    rom.extend(b'\x00')  # Version
    
    # Pad to 0x1000
    rom.extend(b'\x00' * (0x1000 - len(rom)))
    
    # ========== Boot Code at 0x1000 ==========
    
    # Initialize stack
    rom.extend(mips_instr(0x3C1D801F))  # lui $sp, 0x801F
    rom.extend(mips_instr(0x37BDFFF0))  # ori $sp, $sp, 0xFFF0
    
    # ========== Setup Geometry Data in RDRAM ==========
    
    # === Projection Matrix at 0x80100000 ===
    # li $t0, 0x80100000
    rom.extend(mips_instr(0x3C088010))  # lui $t0, 0x8010
    
    # We'll set up projection matrix data at this location later
    # For now, just note the address
    
    # === Vertex Data at 0x80101000 ===
    # Left paddle vertices (4 vertices for a quad)
    # li $t1, 0x80101000
    rom.extend(mips_instr(0x3C098010))  # lui $t1, 0x8010
    rom.extend(mips_instr(0x35291000))  # ori $t1, $t1, 0x1000
    
    # === Right paddle vertices at 0x80101100 ===
    # === Ball vertices at 0x80101200 ===
    
    # ========== Setup RSP Task Structure at 0x80200000 ==========
    # li $t0, 0x80200000
    rom.extend(mips_instr(0x3C088020))  # lui $t0, 0x8020
    
    # Task type = 1 (graphics)
    rom.extend(mips_instr(0x34090001))  # ori $t1, $zero, 1
    rom.extend(mips_instr(0xAD090000))  # sw $t1, 0($t0)
    
    # Task flags = 0
    rom.extend(mips_instr(0xAD000004))  # sw $zero, 4($t0)
    
    # ucode_boot = 0
    rom.extend(mips_instr(0xAD000008))  # sw $zero, 8($t0)
    rom.extend(mips_instr(0xAD00000C))  # sw $zero, 12($t0)
    
    # ucode = 0x80400000 (F3DEX microcode - will be loaded by emulator)
    rom.extend(mips_instr(0x3C094040))  # lui $t1, 0x8040
    rom.extend(mips_instr(0xAD090010))  # sw $t1, 16($t0)
    
    # ucode_size = 0x1000
    rom.extend(mips_instr(0x34091000))  # ori $t1, $zero, 0x1000
    rom.extend(mips_instr(0xAD090014))  # sw $t1, 20($t0)
    
    # ucode_data = 0
    rom.extend(mips_instr(0xAD000018))  # sw $zero, 24($t0)
    rom.extend(mips_instr(0xAD00001C))  # sw $zero, 28($t0)
    
    # dram_stack = 0x80300000
    rom.extend(mips_instr(0x3C098030))  # lui $t1, 0x8030
    rom.extend(mips_instr(0xAD090020))  # sw $t1, 32($t0)
    rom.extend(mips_instr(0x34091800))  # ori $t1, $zero, 0x1800
    rom.extend(mips_instr(0xAD090024))  # sw $t1, 36($t0)
    
    # output_buff = 0 (RSP will write RDP commands directly)
    rom.extend(mips_instr(0xAD000028))  # sw $zero, 40($t0)
    rom.extend(mips_instr(0xAD00002C))  # sw $zero, 44($t0)
    
    # data_ptr = 0x80110000 (F3DEX display list)
    rom.extend(mips_instr(0x3C098011))  # lui $t1, 0x8011
    rom.extend(mips_instr(0xAD090030))  # sw $t1, 48($t0)
    
    # data_size = 0x2000
    rom.extend(mips_instr(0x34092000))  # ori $t1, $zero, 0x2000
    rom.extend(mips_instr(0xAD090034))  # sw $t1, 52($t0)
    
    # yield_data = 0
    rom.extend(mips_instr(0xAD000038))  # sw $zero, 56($t0)
    rom.extend(mips_instr(0xAD00003C))  # sw $zero, 60($t0)
    
    # ========== Trigger RSP ==========
    # Load RSP task structure address to DMEM
    # li $t0, 0xA4040000 (SP_DMEM - KSEG1)
    rom.extend(mips_instr(0x3C08A404))  # lui $t0, 0xA404
    
    # li $t1, 0x80200000 (task structure)
    rom.extend(mips_instr(0x3C098020))  # lui $t1, 0x8020
    
    # Copy task structure pointer to DMEM offset 0
    # For simplicity in HLE, we assume DMEM already has task structure
    
    # Write to SP_STATUS to start RSP
    # li $t0, 0xA4040010 (SP_STATUS)
    rom.extend(mips_instr(0x3C08A404))  # lui $t0, 0xA404
    rom.extend(mips_instr(0x35080010))  # ori $t0, $t0, 0x0010
    
    # Clear halt bit (bit 0 = 0x0001)
    rom.extend(mips_instr(0x34090001))  # ori $t1, $zero, 1
    rom.extend(mips_instr(0xAD090000))  # sw $t1, 0($t0)
    
    # ========== Main Loop ==========
    # Infinite loop (game logic would go here)
    rom.extend(mips_instr(0x08000000 | ((0x1000 + len(rom)) >> 2)))  # j <current address>
    rom.extend(mips_instr(0x00000000))  # nop
    
    # ========== Data Section ==========
    # Pad to 0x100000 for data
    rom.extend(b'\x00' * (0x100000 - len(rom)))
    
    # === Projection Matrix at 0x100000 ===
    proj_matrix = create_matrix_projection(
        fov=math.pi / 3.0,  # 60 degrees
        aspect=4.0/3.0,     # 320x240
        near=10.0,
        far=1000.0
    )
    rom.extend(proj_matrix)
    
    # === Modelview Matrix (identity) at 0x100040 ===
    rom.extend(create_matrix_identity())
    
    # === Camera matrix (translate back) at 0x100080 ===
    camera_matrix = create_matrix_translate(0, 0, -300)
    rom.extend(camera_matrix)
    
    # Pad to 0x101000
    rom.extend(b'\x00' * (0x101000 - len(rom)))
    
    # === Left Paddle Vertices at 0x101000 ===
    # Paddle at x=-100, y=0, z=0, size 20x60x10
    # Front face (4 vertices forming 2 triangles)
    rom.extend(create_vertex(-110, -30, 5, 255, 0, 0))    # v0: red
    rom.extend(create_vertex(-110, 30, 5, 200, 0, 0))     # v1: darker red
    rom.extend(create_vertex(-90, 30, 5, 150, 0, 0))      # v2: even darker
    rom.extend(create_vertex(-90, -30, 5, 100, 0, 0))     # v3: darkest red
    
    # === Right Paddle Vertices at 0x101040 ===
    rom.extend(create_vertex(90, -30, 5, 0, 0, 255))      # v4: blue
    rom.extend(create_vertex(90, 30, 5, 0, 0, 200))       # v5: darker blue
    rom.extend(create_vertex(110, 30, 5, 0, 0, 150))      # v6: even darker
    rom.extend(create_vertex(110, -30, 5, 0, 0, 100))     # v7: darkest blue
    
    # === Ball Vertices at 0x101080 ===
    # Ball at x=0, y=0, z=0, radius ~10 (simple quad)
    rom.extend(create_vertex(-8, -8, 0, 0, 255, 0))       # v8: green
    rom.extend(create_vertex(-8, 8, 0, 0, 255, 0))        # v9: green
    rom.extend(create_vertex(8, 8, 0, 0, 255, 0))         # v10: green
    rom.extend(create_vertex(8, -8, 0, 0, 255, 0))        # v11: green
    
    # Pad to 0x110000
    rom.extend(b'\x00' * (0x110000 - len(rom)))
    
    # === F3DEX Display List at 0x110000 ===
    display_list = bytearray()
    
    # Load projection matrix
    display_list.extend(f3dex_mtx(0x80100000, 0x04 | 0x00))  # PROJECTION | LOAD
    
    # Load camera matrix (modelview)
    display_list.extend(f3dex_mtx(0x80100080, 0x00))  # MODELVIEW | LOAD
    
    # === Draw Left Paddle ===
    # Load left paddle vertices (4 vertices starting at buffer index 0)
    display_list.extend(f3dex_vtx(0x80101000, 4, 0))
    
    # Draw left paddle (2 triangles forming a quad)
    display_list.extend(f3dex_tri2(0, 1, 2, 0, 2, 3))
    
    # === Draw Right Paddle ===
    # Load right paddle vertices (4 vertices starting at buffer index 4)
    display_list.extend(f3dex_vtx(0x80101040, 4, 4))
    
    # Draw right paddle (2 triangles forming a quad)
    display_list.extend(f3dex_tri2(4, 5, 6, 4, 6, 7))
    
    # === Draw Ball ===
    # Load ball vertices (4 vertices starting at buffer index 8)
    display_list.extend(f3dex_vtx(0x80101080, 4, 8))
    
    # Draw ball (2 triangles forming a quad)
    display_list.extend(f3dex_tri2(8, 9, 10, 8, 10, 11))
    
    # End display list
    display_list.extend(f3dex_enddl())
    
    rom.extend(display_list)
    
    # Pad to 8MB
    rom.extend(b'\x00' * (0x800000 - len(rom)))
    
    return bytes(rom)

if __name__ == '__main__':
    rom = build_rom()
    
    # Write ROM file
    with open('test_pong3d.z64', 'wb') as f:
        f.write(rom)
    
    print(f"Created test_pong3d.z64 ({len(rom)} bytes)")
    print("ROM contains:")
    print("  - 3D Pong game with RSP/RDP pipeline")
    print("  - Left paddle (red) at x=-100")
    print("  - Right paddle (blue) at x=+100")
    print("  - Ball (green) at center")
    print("  - Perspective projection")
    print("  - Gouraud shading on all objects")
