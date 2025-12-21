.segment "HEADER"
    .byte "NES", $1A, 2, 1, 0, 0
    .res 8, 0

.segment "VECTORS"
    .word 0, RESET, 0

.segment "CODE"
RESET:
    sei
    cld
    
    ; Wait for PPU
    bit $2002
:   bit $2002
    bpl :-
    
    ; Write palette
    lda #$3F
    sta $2006
    lda #$00
    sta $2006
    lda #$0F        ; Color 0: Black
    sta $2007
    lda #$30        ; Color 1: White
    sta $2007
    lda #$16        ; Color 2: Red
    sta $2007
    lda #$11        ; Color 3: Blue
    sta $2007
    
    ; Fill nametable with tile 1
    lda #$20
    sta $2006
    lda #$00
    sta $2006
    ldx #4
:   ldy #0
:   lda #$01
    sta $2007
    dey
    bne :-
    dex
    bne :--
    
    ; Enable rendering
    lda #$0A        ; Just show BG
    sta $2001
    
:   jmp :-

.segment "CHR"
    ; Tile 0: blank
    .res 16, $00
    ; Tile 1: solid (both planes = 11 = color 3)
    .res 8, $FF
    .res 8, $FF
    ; Rest
    .res 8192 - 32, $00
