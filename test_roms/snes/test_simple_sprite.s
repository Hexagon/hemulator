; SNES Simple Sprite Test ROM
; Purpose: Minimal test to verify basic OBJ/sprite rendering
; Expected: Single 8x8 sprite visible on screen at position (100, 100)

.p816                       ; 65816 processor
.a8                         ; 8-bit accumulator by default
.i8                         ; 8-bit index registers by default

.segment "HEADER"
    ; SNES ROM header (internal header at $FFB0-$FFDF for LoROM)
    ; Title (21 bytes)
    .byte "SIMPLE SPRITE TEST   "
    
    ; ROM makeup byte
    .byte $20               ; LoROM, slow speed
    
    ; ROM type (ROM only, no RAM or special chips)
    .byte $00
    
    ; ROM size (32KB = $08)
    .byte $08
    
    ; RAM size (no RAM)
    .byte $00
    
    ; Country code (01 = USA)
    .byte $01
    
    ; Developer ID (33 = Extended header)
    .byte $33
    
    ; Version number
    .byte $00
    
    ; Checksum complement
    .word $0000
    
    ; Checksum
    .word $0000
    
.segment "VECTORS"
    ; Native mode vectors ($FFE0-$FFEF)
    .word $0000             ; $FFE0 - unused
    .word $0000             ; $FFE2 - unused
    .word NMI               ; $FFE4 - COP (reuse NMI)
    .word $0000             ; $FFE6 - BRK (unused)
    .word $0000             ; $FFE8 - ABORT (unused)
    .word NMI               ; $FFEA - NMI
    .word $0000             ; $FFEC - reserved
    .word IRQ               ; $FFEE - IRQ
    
    ; Emulation mode vectors ($FFF0-$FFFF)
    .word $0000             ; $FFF0 - unused
    .word $0000             ; $FFF2 - unused  
    .word NMI               ; $FFF4 - COP (reuse NMI)
    .word $0000             ; $FFF6 - reserved
    .word $0000             ; $FFF8 - ABORT (unused)
    .word NMI               ; $FFFA - NMI
    .word RESET             ; $FFFC - RESET (entry point!)
    .word IRQ               ; $FFFE - IRQ/BRK

.segment "CODE"

RESET:
    sei                     ; Disable interrupts
    clc
    xce                     ; Switch to native mode (clear emulation flag)
    
    rep #$10                ; 16-bit index registers
    .i16
    sep #$20                ; 8-bit accumulator
    .a8
    
    ; Set up stack
    ldx #$1FFF
    txs
    
    ; Wait for VBlank
    lda $4212
:   lda $4212
    and #$80
    beq :-
    
    ; Turn off screen (enable force blank for safe VRAM access)
    lda #$80
    sta $2100               ; INIDISP: Force blank
    
    ; Set up VRAM for sprite data upload
    ; VRAM auto-increment on $2119 write
    lda #$80
    sta $2115               ; VRAM increment mode: +1 on high byte write
    
    ; Upload sprite tile data to VRAM $0000 (word address)
    ; This will be the OBJ base address (name_base=0 means VRAM $0000)
    ldx #$0000
    stx $2116               ; VRAM address = $0000
    
    ; Upload a simple 8x8 sprite tile (4bpp = 32 bytes)
    ; SNES VRAM is 16-bit word-based, so we write 2 bytes at a time
    ldx #$0000
UPLOAD_SPRITE_DATA:
    lda SPRITE_TILE_DATA,x
    sta $2118               ; Write low byte
    inx
    lda SPRITE_TILE_DATA,x
    sta $2119               ; Write high byte (triggers increment)
    inx
    cpx #32
    bne UPLOAD_SPRITE_DATA
    
    ; Set up sprite palette (CGRAM)
    ; Sprites use palettes 128-255 (8 palettes of 16 colors each)
    ; Set CGRAM address for sprite palette 0 (starts at color 128)
    lda #$80                ; Color 128
    sta $2121
    
    ; Upload colors for sprite palette 0
    ; Color 0 (128): Transparent
    lda #$00
    sta $2122               ; Low byte
    sta $2122               ; High byte
    
    ; Color 1 (129): Bright red for visibility
    lda #$1F                ; Red: bits 0-4
    sta $2122               ; Low byte
    lda #$00                ; Green: bits 5-9, Blue: bits 10-14
    sta $2122               ; High byte
    
    ; Fill remaining colors with white for testing
    ldx #2
UPLOAD_SPRITE_PALETTE:
    lda #$FF
    sta $2122
    lda #$7F
    sta $2122
    inx
    cpx #16
    bne UPLOAD_SPRITE_PALETTE
    
    ; Set up OAM (sprite attribute memory)
    ; OAM has 512 bytes for sprite attributes (128 sprites × 4 bytes)
    ; + 32 bytes for high table (X MSB and size bits)
    
    lda #$00
    sta $2102               ; OAM address low
    sta $2103               ; OAM address high
    
    ; Sprite 0: X=100, Y=100, tile=0, palette=0
    lda #100
    sta $2104               ; X position (low 8 bits)
    lda #100
    sta $2104               ; Y position
    lda #$00
    sta $2104               ; Tile number
    lda #$00                ; Bits: palette=0 (bits 1-3), priority=0 (bits 4-5), 
                            ;       no flip (bits 6-7), nameselect=0 (bit 0)
    sta $2104               ; Attributes
    
    ; Clear remaining sprites (move them off-screen at Y=240)
    ldx #1
CLEAR_SPRITES:
    lda #$F0                ; Y=240 (off-screen)
    sta $2104               ; X
    sta $2104               ; Y
    lda #$00
    sta $2104               ; Tile
    sta $2104               ; Attr
    inx
    cpx #128
    bne CLEAR_SPRITES
    
    ; Set up OAM high table (32 bytes, 4 sprites per byte)
    ; Bits 0,2,4,6: X MSB (bit 8 of X coordinate) for sprites 0-3
    ; Bits 1,3,5,7: Size bit for sprites 0-3 (0=small, 1=large)
    lda #$00                ; Sprites 0-3: all X MSB=0, size=small
    sta $2104
    
    ; Clear rest of high table (31 more bytes)
    ldx #1
CLEAR_OAM_HIGH:
    lda #$55                ; Pattern: alternating bits for off-screen sprites
    sta $2104
    inx
    cpx #32
    bne CLEAR_OAM_HIGH
    
    ; Set OBSEL register (sprite tile base address and size)
    ; Bits 0-2: name_base (base address = name_base << 14)
    ;           We want VRAM $0000, so name_base=0
    ; Bits 3-4: name_select (gap between sprite pages)
    ; Bits 5-7: sprite size selection (0 = 8x8 and 16x16)
    lda #$00                ; name_base=0, name_select=0, size=0
    sta $2101               ; OBSEL register
    
    ; Enable sprites ONLY on main screen (to replicate SMW's TM=0x10)
    lda #$10
    sta $212C               ; TM register: bit 4 = OBJ layer
    
    ; Turn on screen (brightness 15, no force blank)
    lda #$0F
    sta $2100               ; INIDISP: brightness=15, force_blank=0
    
MAIN_LOOP:
    wai                     ; Wait for interrupt (NMI)
    jmp MAIN_LOOP

NMI:
    rti

IRQ:
    rti

; Sprite tile data: 8x8 4bpp sprite (32 bytes total)
; SNES 4bpp format: SEQUENTIAL bitplanes (like BG tiles)
; Layout: BP0[rows 0-7], BP1[rows 0-7], BP2[rows 0-7], BP3[rows 0-7]
; Each bitplane byte has MSB = leftmost pixel
; We'll make a solid square using color index 1 (BP0=1, BP1=0, BP2=0, BP3=0)
SPRITE_TILE_DATA:
    ; Bitplane 0 (8 bytes, one per row)
    .byte $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF
    
    ; Bitplane 1 (8 bytes, one per row)
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    
    ; Bitplane 2 (8 bytes, one per row)
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    
    ; Bitplane 3 (8 bytes, one per row)
    .byte $00, $00, $00, $00, $00, $00, $00, $00
