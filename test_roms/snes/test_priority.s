; SNES Priority Test ROM
; Purpose: Test BG tile priority bit handling
; Expected: High-priority tiles should render in front of low-priority tiles

.p816                       ; 65816 processor
.a8                         ; 8-bit accumulator by default
.i8                         ; 8-bit index registers by default

.segment "HEADER"
    ; SNES ROM header (internal header at $FFB0-$FFDF for LoROM)
    ; Title (21 bytes)
    .byte "PRIORITY TEST ROM    "
    
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
    xce                     ; Switch to native mode
    
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
    
    ; Enable force blank
    lda #$80
    sta $2100
    
    ; Set Mode 0
    stz $2105
    
    ; BG1 tilemap at $0000, CHR at $1000
    stz $2107               ; BG1 tilemap
    lda #$01
    sta $210B               ; BG1 CHR
    
    ; Enable BG1
    lda #$01
    sta $212C
    
    ; Set up palette
    stz $2121               ; CGRAM address = 0
    
    ; Color 0: Black
    stz $2122
    stz $2122
    
    ; Color 1: Red
    lda #$1F
    sta $2122
    stz $2122
    
    ; Color 2: Green  
    lda #$E0
    sta $2122
    lda #$03
    sta $2122
    
    ; Color 3: Blue
    stz $2122
    lda #$7C
    sta $2122
    
    ; Upload tile data to VRAM $1000
    ldx #$1000
    stx $2116
    
    ; Tile 0: Color 1 (red)
    lda #$FF
    ldx #$0008
:   sta $2118
    stz $2119
    dex
    bne :-
    
    ; Tile 1: Color 3 (blue)
    lda #$FF
    ldx #$0008
:   sta $2118
    sta $2119
    dex
    bne :-
    
    ; Fill tilemap
    stz $2116
    stz $2117
    
    rep #$20                ; 16-bit accumulator
    .a16
    
    ; Fill screen with alternating tiles
    ldx #$0000
:   txa
    and #$0001              ; Alternate tiles
    beq LowPriTile
    
    ; High priority tile (tile 1, priority bit set)
    lda #$2001              ; Tile 1, priority bit (bit 13)
    jmp WriteTile
    
LowPriTile:
    ; Low priority tile (tile 0, no priority)
    lda #$0000              ; Tile 0, no priority
    
WriteTile:
    sta $2118
    inx
    cpx #$0400              ; 32x32 = 1024 tiles
    bne :-
    
    sep #$20                ; 8-bit accumulator
    .a8
    
    ; Disable force blank
    lda #$0F
    sta $2100
    
    ; Enable NMI
    lda #$80
    sta $4200
    
    ; Main loop
    cli
:   wai
    jmp :-

NMI:
    rti

IRQ:
    rti

