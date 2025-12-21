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
    
    ; Tile 0: White
    ld hl, $8000
    xor a
    ld b, 16
.t0:
    ld [hl+], a
    dec b
    jr nz, .t0
    
    ; Tile 1: Dark (all $FF)
    ld hl, $8010
    xor a
    ld b, 8
.t1a:
    ld [hl+], a
    dec b
    jr nz, .t1a
    ld a, $FF
    ld b, 8
.t1b:
    ld [hl+], a
    dec b
    jr nz, .t1b
    
    ; Fill tilemap: alternating 0 and 1
    ld hl, $9800
    ld c, 18               ; rows
.row:
    ld b, 20               ; cols
    ld a, c
    and 1
.col:
    ld [hl+], a
    xor 1
    dec b
    jr nz, .col
    dec c
    jr nz, .row
    
    ld a, %11100100
    ldh [$FF47], a
    
    xor a
    ldh [$FF42], a
    ldh [$FF43], a
    
    ld a, %10000001
    ldh [$FF40], a
    
.loop:
    halt
    jr .loop
