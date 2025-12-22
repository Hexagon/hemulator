; Game Boy test ROM - checkerboard pattern

SECTION "Header", ROM0[$100]
    nop
    jp Start

    DB $CE,$ED,$66,$66,$CC,$0D,$00,$0B,$03,$73,$00,$83,$00,$0C,$00,$0D
    DB $00,$08,$11,$1F,$88,$89,$00,$0E,$DC,$CC,$6E,$E6,$DD,$DD,$D9,$99
    DB $BB,$BB,$67,$63,$6E,$0E,$EC,$CC,$DD,$DC,$99,$9F,$BB,$B9,$33,$3E

    DB "GBTEST"
    DS $143 - @, 0
    DB $00, $00, $00, $01, $00, $00, $00
    DW $0000

SECTION "Code", ROM0[$150]

Start:
    di
    
.waitVBlank:
    ldh a, [$FF44]
    cp 144
    jr c, .waitVBlank
    
    xor a
    ldh [$FF40], a
    
    ; Tile 0: White (color index 0 = bitplane0=0, bitplane1=0)
    ld hl, $8000
    xor a                  ; a = 0
    ld b, 16
.t0:
    ld [hl+], a            ; Fill 16 bytes with 0
    dec b
    jr nz, .t0
    
    ; Tile 1: Dark gray (color index 1 = bitplane0=1, bitplane1=0)
    ; For 2bpp: bits set in bitplane 0 only = color index 1
    ; Each row needs: bitplane0=$FF, bitplane1=$00 (interleaved)
    ld hl, $8010
    ld b, 8                ; 8 rows
.t1_row:
    ld a, $FF
    ld [hl+], a            ; Bitplane 0: all 1s
    xor a
    ld [hl+], a            ; Bitplane 1: all 0s
    dec b
    jr nz, .t1_row
    
    ; Fill tilemap: alternating 0 and 1 in checkerboard pattern
    ld hl, $9800
    ld d, 0                ; d = row counter (0-17)
.row:
    ld b, 20               ; cols (Full screen is 20 tiles horizontally)
    ld a, d
    and 1                  ; Start pattern based on row number (ensures checkerboard)
.col:
    ld [hl+], a
    xor 1                  ; Toggle between 0 and 1
    dec b
    jr nz, .col
    inc d
    ld a, d
    cp 18                  ; Check if we've done 18 rows
    jr nz, .row
    
    ; Set palette: Map color 0 to white, color 1 to dark gray (as "red" - best we can do on DMG)
    ; Palette format: bits [7:6]=color3, [5:4]=color2, [3:2]=color1, [1:0]=color0
    ; We want: color 0→white(0b00), color 1→dark gray(0b10), color 2→dark gray(0b10), color 3→black(0b11)
    ; So: 0b11101000 = 0xE8
    ; Note: DMG only has 4 shades of gray, so we use dark gray to represent "red"
    ld a, %11101000
    ldh [$FF47], a
    
    xor a
    ldh [$FF42], a
    ldh [$FF43], a
    
    ld a, %10010001        ; Bit 7=LCD on, Bit 4=BG tiles at $8000 (unsigned), Bit 0=BG enabled
    ldh [$FF40], a
    
.loop:
    halt
    jr .loop
