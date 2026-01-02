; Enhanced SNES test ROM - More comprehensive testing
; Purpose: Test Mode 1, sprites, priority, NMI handling to match real game behavior
; This ROM tests features that commercial games commonly use

.p816                       ; 65816 processor
.a8                         ; 8-bit accumulator by default
.i8                         ; 8-bit index registers by default

.segment "HEADER"
    ; SNES ROM header (internal header at $FFB0-$FFDF for LoROM)
    ; Title (21 bytes)
    .byte "ENHANCED SNES TEST  "
    
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

; Variables in WRAM (using equates instead of defines)
NMI_COUNT = $7E0000   ; Count NMIs to verify they work
INIT_DONE = $7E0001   ; Flag to indicate initialization is complete

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
    
    ; Initialize variables
    lda #$00
    sta NMI_COUNT
    sta INIT_DONE
    
    ; Wait for VBlank before initialization
    lda $4212
:   lda $4212
    and #$80
    beq :-
    
    ; Turn off screen (force blank)
    lda #$80
    sta $2100               ; Force blank
    
    ; Set up PPU registers for Mode 1 (most common commercial mode)
    ; Mode 1: BG1/BG2 are 4bpp (16 colors), BG3 is 2bpp (4 colors)
    lda #$01
    sta $2105               ; BG mode 1, BG3 priority off
    
    ; Set BG1 tilemap to VRAM $0000, size 32x32
    lda #$00
    sta $2107               ; BG1 tilemap address
    
    ; Set BG2 tilemap to VRAM $0400, size 32x32
    lda #$08
    sta $2108               ; BG2 tilemap address
    
    ; Set BG3 tilemap to VRAM $0800, size 32x32
    lda #$10
    sta $2109               ; BG3 tilemap address
    
    ; Set BG1 CHR to VRAM $1000, BG2 CHR to VRAM $2000
    lda #$12
    sta $210B               ; BG1 CHR = $1000, BG2 CHR = $2000
    
    ; Set BG3 CHR to VRAM $3000
    lda #$03
    sta $210C               ; BG3 CHR = $3000
    
    ; Set sprite size to 8x8/16x16
    lda #$02                ; Size mode 0, base at $6000, no offset
    sta $2101               ; OBSEL
    
    ; Enable all layers and sprites on main screen
    lda #$17                ; BG1, BG2, BG3, and OBJ enabled
    sta $212C               ; Main screen designation
    
    ; Set up palette - Create distinct colors for testing
    stz $2121               ; CGRAM address = 0
    
    ; BG1 Palette 0 (colors 0-15)
    ; Color 0: Black (transparent backdrop)
    stz $2122
    stz $2122
    
    ; Color 1: White
    lda #$FF
    sta $2122
    lda #$7F
    sta $2122
    
    ; Color 2: Red
    lda #$1F
    sta $2122
    stz $2122
    
    ; Color 3: Green
    stz $2122
    lda #$03
    sta $2122
    
    ; Color 4: Blue
    stz $2122
    lda #$7C
    sta $2122
    
    ; Colors 5-15: Fill with variations
    ldx #$000B              ; 11 more colors
:   lda #$FF
    sta $2122
    lda #$00
    sta $2122
    dex
    bne :-
    
    ; BG2 Palette 0 (colors 16-31)
    ; Color 0: Transparent
    stz $2122
    stz $2122
    
    ; Color 1: Yellow
    lda #$FF
    sta $2122
    lda #$03
    sta $2122
    
    ; Color 2: Cyan
    lda #$E0
    sta $2122
    lda #$7C
    sta $2122
    
    ; Color 3: Magenta
    lda #$1F
    sta $2122
    lda #$7C
    sta $2122
    
    ; Fill rest with gray
    ldx #$000C              ; 12 more colors
:   lda #$94
    sta $2122
    lda #$52
    sta $2122
    dex
    bne :-
    
    ; BG3 Palette 0 (colors 32-35, 2bpp)
    ; Color 0: Transparent
    stz $2122
    stz $2122
    
    ; Color 1: Orange
    lda #$1F
    sta $2122
    lda #$02
    sta $2122
    
    ; Color 2: Purple
    lda #$10
    sta $2122
    lda #$60
    sta $2122
    
    ; Color 3: Light blue
    lda #$E0
    sta $2122
    lda #$3E
    sta $2122
    
    ; Sprite palette (colors 128-143)
    lda #$80                ; Start at color 128
    sta $2121
    
    ; Color 0: Transparent
    stz $2122
    stz $2122
    
    ; Color 1: Bright white
    lda #$FF
    sta $2122
    lda #$7F
    sta $2122
    
    ; Color 2: Bright red
    lda #$1F
    sta $2122
    stz $2122
    
    ; Color 3: Bright green
    stz $2122
    lda #$03
    sta $2122
    
    ; Fill rest with bright colors
    ldx #$000C
:   lda #$FF
    sta $2122
    lda #$7F
    sta $2122
    dex
    bne :-
    
    ; Upload tile data to VRAM
    ; BG1 CHR at $1000 (4bpp tiles)
    ldx #$1000
    stx $2116               ; VRAM address
    
    ; Tile 0: Solid color 1 (white) - 4bpp
    ; Each row: 4 bytes (bitplane 0, 1, 2, 3)
    ; For color 1: bitplane 0 = $FF, others = $00
    ldy #$0008              ; 8 rows
:   lda #$FF                ; Bitplane 0
    sta $2118
    stz $2119               ; Bitplane 1
    stz $2118               ; Bitplane 2
    stz $2119               ; Bitplane 3
    dey
    bne :-
    
    ; Tile 1: Solid color 2 (red) - 4bpp
    ldy #$0008
:   stz $2118               ; Bitplane 0
    lda #$FF
    sta $2119               ; Bitplane 1
    stz $2118               ; Bitplane 2
    stz $2119               ; Bitplane 3
    dey
    bne :-
    
    ; Tile 2: Solid color 4 (blue) - 4bpp
    ldy #$0008
:   stz $2118               ; Bitplane 0
    stz $2119               ; Bitplane 1
    stz $2118               ; Bitplane 2
    lda #$FF
    sta $2119               ; Bitplane 3
    dey
    bne :-
    
    ; BG2 CHR at $2000 (4bpp tiles)
    ldx #$2000
    stx $2116
    
    ; Tile 0: Solid color 1 (yellow) - 4bpp
    ldy #$0008
:   lda #$FF
    sta $2118
    stz $2119
    stz $2118
    stz $2119
    dey
    bne :-
    
    ; Tile 1: Solid color 2 (cyan) - 4bpp
    ldy #$0008
:   stz $2118
    lda #$FF
    sta $2119
    stz $2118
    stz $2119
    dey
    bne :-
    
    ; BG3 CHR at $3000 (2bpp tiles)
    ldx #$3000
    stx $2116
    
    ; Tile 0: Solid color 3 (light blue) - 2bpp
    ldy #$0008
:   lda #$FF                ; Bitplane 0
    sta $2118
    lda #$FF                ; Bitplane 1
    sta $2119
    dey
    bne :-
    
    ; Sprite CHR at $6000 (4bpp, for sprites)
    ldx #$6000
    stx $2116
    
    ; Sprite tile 0: Solid bright white
    ldy #$0008
:   lda #$FF
    sta $2118
    stz $2119
    stz $2118
    stz $2119
    dey
    bne :-
    
    ; Sprite tile 1: Solid bright red
    ldy #$0008
:   stz $2118
    lda #$FF
    sta $2119
    stz $2118
    stz $2119
    dey
    bne :-
    
    ; Upload BG1 tilemap (at $0000)
    ldx #$0000
    stx $2116
    
    ; Create a simple pattern: horizontal stripes
    ; Top 8 rows: tile 0 (white)
    ; Middle 8 rows: tile 1 (red)
    ; Bottom 8 rows: tile 2 (blue)
    ldy #$0000              ; Row counter
row_loop_bg1:
    ldx #$0000              ; Column counter
col_loop_bg1:
    ; Determine tile based on row
    lda $7E0002             ; Load Y (row counter) from WRAM
    cmp #$08
    bcc use_tile0
    cmp #$10
    bcc use_tile1
    lda #$02                ; Tile 2 (blue)
    jmp write_tile_bg1
use_tile1:
    lda #$01                ; Tile 1 (red)
    jmp write_tile_bg1
use_tile0:
    lda #$00                ; Tile 0 (white)
write_tile_bg1:
    sta $2118               ; Write tile number
    stz $2119               ; Write attributes (no flip, palette 0, priority 0)
    
    inx
    cpx #$0020              ; 32 columns
    bne col_loop_bg1
    
    ; Save row counter before incrementing
    tya
    sta $7E0002
    iny
    cpy #$001C              ; 28 rows (224/8)
    bne row_loop_bg1
    
    ; Upload BG2 tilemap (at $0400)
    ldx #$0400
    stx $2116
    
    ; Create vertical stripes for BG2
    ldy #$0000
row_loop_bg2:
    ldx #$0000
col_loop_bg2:
    ; Alternate tiles based on column
    txa
    and #$01
    sta $2118               ; Tile 0 or 1
    stz $2119               ; No attributes
    
    inx
    cpx #$0020
    bne col_loop_bg2
    
    iny
    cpy #$001C
    bne row_loop_bg2
    
    ; Upload BG3 tilemap (at $0800)
    ldx #$0800
    stx $2116
    
    ; Fill BG3 with tile 0 (light blue background)
    ldy #$0380              ; 32*28 tiles
:   stz $2118
    stz $2119
    dey
    bne :-
    
    ; Set up sprites in OAM
    ; Sprite 0: at position (64, 64), tile 0, palette 0
    stz $2102               ; OAM address low
    stz $2103               ; OAM address high
    
    lda #64
    sta $2104               ; X position
    lda #64
    sta $2104               ; Y position
    lda #$00
    sta $2104               ; Tile number
    lda #$00                ; Palette 0, priority 0, no flip
    sta $2104               ; Attributes
    
    ; Sprite 1: at position (128, 64), tile 1, palette 0
    lda #128
    sta $2104               ; X position
    lda #64
    sta $2104               ; Y position
    lda #$01
    sta $2104               ; Tile number
    lda #$00
    sta $2104               ; Attributes
    
    ; Fill rest of OAM with off-screen sprites
    ldx #$01FC              ; 508 more bytes (127 sprites * 4 bytes)
:   lda #$F0                ; Off-screen Y
    sta $2104
    dex
    bne :-
    
    ; Set OAM high table (size bits, X MSB)
    ; First sprite: small size (8x8)
    stz $2104               ; Bits 0-1: sprite 0-3 size/X MSB
    
    ; Fill rest of high table
    ldx #$001F              ; 31 more bytes
:   stz $2104
    dex
    bne :-
    
    ; Enable NMI
    lda #$81                ; Enable NMI (bit 7) and auto-joypad read (bit 0)
    sta $4200
    
    ; Mark initialization as done
    lda #$01
    sta INIT_DONE
    
    ; Turn on screen (brightness = 15)
    lda #$0F
    sta $2100
    
    cli                     ; Enable interrupts

main_loop:
    wai                     ; Wait for NMI
    
    ; Simple animation: increment BG1 scroll every frame
    lda NMI_COUNT
    and #$01                ; Only update every other frame
    bne main_loop
    
    ; Scroll BG1 horizontally
    lda $7E0010             ; Load scroll offset
    inc
    sta $7E0010             ; Store back
    sta $210D               ; Write to BG1HOFS
    stz $210D               ; Write high byte
    
    jmp main_loop

NMI:
    ; Save registers
    pha
    phx
    phy
    
    ; Increment NMI counter
    lda NMI_COUNT
    inc
    sta NMI_COUNT
    
    ; Read $4210 to acknowledge NMI
    lda $4210
    
    ; Restore registers
    ply
    plx
    pla
    rti

IRQ:
    rti
