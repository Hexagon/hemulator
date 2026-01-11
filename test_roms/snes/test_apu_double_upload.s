; SNES APU Double Upload Test ROM
; Purpose: Simulate commercial game behavior with two APU upload sequences
; This tests the timing-critical handshake protocol that real games use

.p816                       ; 65816 processor
.a8                         ; 8-bit accumulator by default
.i8                         ; 8-bit index registers by default

.segment "HEADER"
    ; SNES ROM header (internal header at $FFB0-$FFDF for LoROM)
    ; Title (21 bytes)
    .byte "APU DOUBLE UPLOAD   "
    
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
    
    ; Initialize PPU (force blank during setup)
    lda #$80
    sta $2100               ; Screen off
    
    ; Test 1: Wait for IPL ROM ready signature ($BBAA)
    ; This simulates the first upload handshake
    jsr wait_apu_ready
    
    ; If we got here, first wait succeeded
    ; Write test pattern to show we passed first check
    lda #$01
    sta $0100               ; Memory marker
    
    ; Simulate upload by writing some data to APU ports
    lda #$CC
    sta $2140               ; Send upload start signal
    
    ; Wait a bit (simulate upload time)
    ldx #$0100
delay_loop1:
    dex
    bne delay_loop1
    
    ; Test 2: Clear ports and wait for ready signature AGAIN
    ; This simulates what commercial games do for second upload
    lda #$00
    sta $2140
    sta $2141
    sta $2142
    sta $2143
    
    ; Wait a bit for SPC700 to process
    ldx #$0100
delay_loop2:
    dex
    bne delay_loop2
    
    ; Now wait for ready signature again (THIS IS WHERE REAL GAMES HANG!)
    jsr wait_apu_ready
    
    ; If we got here, second wait succeeded too!
    lda #$02
    sta $0101               ; Memory marker for success
    
    ; Turn on screen to show we succeeded
    lda #$0F
    sta $2100
    
success:
    wai
    jmp success

; Wait for APU ready signature ($BBAA in ports $2140/$2141)
; This is the critical timing-sensitive operation
wait_apu_ready:
    .a8
    .i16
    php
    rep #$30                ; 16-bit A and X/Y
    .a16
    
wait_loop:
    lda #$BBAA              ; Expected ready signature
    cmp $2140               ; Read ports $2140/$2141 as 16-bit
    bne wait_loop           ; Loop until we see $BBAA
    
    sep #$20                ; Back to 8-bit A
    .a8
    plp
    rts

NMI:
    rti

IRQ:
    rti
