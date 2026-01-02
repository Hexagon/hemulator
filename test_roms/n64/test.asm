; N64 test ROM - displays colored rectangles using RDP
; Purpose: Verify basic RDP functionality (display list processing, fill commands)
; Assembles with mips64-gcc or n64 toolchain

.set noreorder
.set noat

; N64 ROM header
.section .text

; Boot code entry point (0x80000000 in cached space, 0x00000000 physical)
start:
    ; Initialize stack pointer
    li      $sp, 0x801FFFF0
    
    ; Build display list in RDRAM starting at 0x00100000
    li      $t0, 0x00100000          ; Display list address in RDRAM
    
    ; Command 1: SET_FILL_COLOR (0x37) - Red (0xFFFF0000)
    li      $t1, 0x37000000          ; SET_FILL_COLOR command
    sw      $t1, 0($t0)              ; Store command word 0
    li      $t1, 0xFFFF0000          ; Red color RGBA
    sw      $t1, 4($t0)              ; Store command word 1
    
    ; Command 2: FILL_RECTANGLE (0x36) - 100x100 rectangle at (50,50)
    ; Coordinates in 10.2 fixed point: 50*4=200(0xC8), 150*4=600(0x258)
    li      $t1, 0x36258258          ; FILL_RECTANGLE cmd + X2,Y2 = 150,150
    sw      $t1, 8($t0)              ; Store command word 0
    li      $t1, 0x000C80C8          ; X1,Y1 = 50,50 (fixed: was 0x00C800C8)
    sw      $t1, 12($t0)             ; Store command word 1
    
    ; Command 3: SET_FILL_COLOR (0x37) - Green (0xFF00FF00)
    li      $t1, 0x37000000
    sw      $t1, 16($t0)
    li      $t1, 0xFF00FF00          ; Green color
    sw      $t1, 20($t0)
    
    ; Command 4: FILL_RECTANGLE (0x36) - 50x50 rectangle at (160,90)
    ; 160*4=640(0x280), 210*4=840(0x348), 90*4=360(0x168), 140*4=560(0x230)
    li      $t1, 0x36348230          ; X2,Y2 = 210,140
    sw      $t1, 24($t0)
    li      $t1, 0x00280168          ; X1,Y1 = 160,90 (fixed: was 0x02800168)
    sw      $t1, 28($t0)
    
    ; Command 5: SYNC_FULL (0x29) - Synchronize
    li      $t1, 0x29000000
    sw      $t1, 32($t0)
    li      $t1, 0x00000000
    sw      $t1, 36($t0)
    
    ; Trigger RDP to process display list
    li      $t0, 0x04100000          ; RDP command register base
    li      $t1, 0x00100000          ; DPC_START address
    sw      $t1, 0($t0)              ; Write DPC_START
    li      $t1, 0x00100028          ; DPC_END address (40 bytes = 5 commands)
    sw      $t1, 4($t0)              ; Write DPC_END (triggers processing)
    
    ; Infinite loop
loop:
    j       loop
    nop

; Pad to required ROM size (minimum 1MB for N64)
.section .data
.space 0x100000 - (. - start)

; IPL3 boot code checksum area (required by N64 boot ROM)
.section .ipl3
.space 0x1000
