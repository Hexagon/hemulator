; Minimal NES test ROM for debugging

.segment "HEADER"
    ; iNES header
    .byte "NES", $1A        ; Magic string
    .byte 2                 ; 2 x 16KB PRG-ROM
    .byte 1                 ; 1 x 8KB CHR-ROM
    .byte $00               ; Mapper 0 (NROM), horizontal mirroring
    .byte $00               ; No special features
    .res 8, 0               ; Padding

.segment "VECTORS"
    .word NMI
    .word RESET
    .word IRQ

.segment "STARTUP"

.segment "CODE"

RESET:
    sei
    cld
    ldx #$FF
    txs
    
    ; Wait for PPU
    bit $2002
:   bit $2002
    bpl :-
:   bit $2002
    bpl :-
    
    ; Set up palette - just one color
    lda #$3F
    sta $2006
    lda #$00
    sta $2006
    lda #$0F            ; Black
    sta $2007
    lda #$30            ; White  
    sta $2007
    
    ; Fill nametable with tile 0
    lda #$20
    sta $2006
    lda #$00
    sta $2006
    ldx #4
:   ldy #0
:   lda #$00
    sta $2007
    dey
    bne :-
    dex
    bne :--
    
    ; Enable rendering
    lda #$1E
    sta $2001
    lda #$80
    sta $2000
    
forever:
    jmp forever

NMI:
    rti

IRQ:
    rti

.segment "RODATA"

.segment "CHR"
    ; Tile $00: Solid color (all 1s)
    .byte $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF
    .byte $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF
    ; Fill rest
    .res 8192 - 16, $00
