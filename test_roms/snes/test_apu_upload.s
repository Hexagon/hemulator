; SNES APU Upload Protocol Test ROM
; Purpose: Simulate the full commercial game APU upload protocol
; This tests the complete handshake including data upload and acknowledgments

.p816                       ; 65816 processor
.a8                         ; 8-bit accumulator by default
.i8                         ; 8-bit index registers by default

.segment "HEADER"
    ; SNES ROM header
    .byte "APU UPLOAD TEST     "
    .byte $20               ; LoROM, slow speed
    .byte $00               ; ROM only
    .byte $08               ; ROM size (32KB)
    .byte $00               ; RAM size
    .byte $01               ; Country code (USA)
    .byte $33               ; Developer ID
    .byte $00               ; Version
    .word $0000             ; Checksum complement
    .word $0000             ; Checksum
    
.segment "VECTORS"
    ; Native mode vectors
    .word $0000, $0000, NMI, $0000, $0000, NMI, $0000, IRQ
    ; Emulation mode vectors
    .word $0000, $0000, NMI, $0000, $0000, NMI, RESET, IRQ

.segment "CODE"

RESET:
    sei
    clc
    xce                     ; Native mode
    rep #$10                ; 16-bit index
    .i16
    sep #$20                ; 8-bit accumulator
    .a8
    ldx #$1FFF
    txs
    
    lda #$80
    sta $2100               ; Screen off
    
    ; ========================================
    ; FIRST UPLOAD: Simulate uploading SPC700 driver
    ; ========================================
    
    ; Step 1: Wait for IPL ROM ready ($BBAA)
    jsr wait_apu_ready
    lda #$01
    sta $0100               ; Marker: passed first ready wait
    
    ; Step 2: Send upload start command ($CC)
    lda #$CC
    sta $2140
    
    ; CRITICAL: Give SPC700 time to process $CC command!
    ; The SPC700 needs ~20 cycles to read $CC, exit wait loop, and branch to upload routine
    ; With clock ratio of 0.286, main CPU needs ~70 cycles
    ldy #$0020              ; Delay loop
:   dey
    bne :-
    
    ; Step 3: Send destination address ($0200) to ports 2/3
    lda #$00
    sta $2142               ; Low byte
    lda #$02
    sta $2143               ; High byte
    
    ; Step 4: Upload some bytes
    ; Note: SPC700 IPL ROM waits for NON-ZERO index at $FFD6-$FFD8
    ; So we start with index 1, not 0
    ldx #$0001              ; Start with index 1, not 0!
upload_loop1:
    ; Send index to port 0
    txa
    sta $2140
    
    ; Send data byte to port 1
    lda test_data, x
    sta $2141
    
    ; Small delay to give SPC700 time to process
    ; (Real games have code here that provides natural delays)
    ldy #$0020              ; Larger delay
:   dey
    bne :-
    
    ; Wait for SPC700 to echo the index
    .a8
:   txa                     ; Get index back into A for comparison
    cmp $2140
    bne :-
    
    inx
    cpx #$0010              ; Upload 15 bytes (indices 1-15)
    bne upload_loop1
    
    lda #$02
    sta $0101               ; Marker: completed first upload
    
    ; Step 5: Signal end of upload (send $00 $00 to ports 0/1)
    lda #$00
    sta $2140
    sta $2141
    
    ; Wait for echo
    .a8
:   lda $2140
    bne :-
    
    lda #$03
    sta $0102               ; Marker: got end-of-upload echo
    
    ; ========================================
    ; SECOND UPLOAD: This is where real games often hang!
    ; ========================================
    
    ; Clear all ports
    lda #$00
    sta $2140
    sta $2141
    sta $2142
    sta $2143
    
    ; Wait for driver to re-enable IPL or provide ready signal
    ; In real hardware, the uploaded driver should handle this
    ; For testing, we're checking if the IPL ROM remains active
    
    ; Small delay
    ldx #$1000
:   dex
    bne :-
    
    ; Try to wait for ready again (THIS IS THE CRITICAL TEST!)
    jsr wait_apu_ready
    
    lda #$04
    sta $0103               ; Marker: passed second ready wait!
    
    ; If we got here, everything worked!
    lda #$0F
    sta $2100               ; Screen on
    
success:
    wai
    jmp success

; Wait for APU ready signature ($BBAA)
wait_apu_ready:
    .a8
    .i16
    php
    rep #$30                ; 16-bit A and X/Y
    .a16
    
    ldx #$0000              ; Timeout counter
wait_loop:
    lda #$BBAA
    cmp $2140               ; Read ports $2140/$2141 as 16-bit
    beq got_ready
    
    inx
    cpx #$8000              ; Timeout after many iterations
    bne wait_loop
    
    ; Timeout! Hang here for debugging
timeout_hang:
    sep #$20
    .a8
    lda #$FF
    sta $0110               ; Timeout marker
    wai
    jmp timeout_hang
    
got_ready:
    sep #$20
    .a8
    plp
    rts

NMI:
    rti

IRQ:
    rti

; Test data to upload
test_data:
    .byte $CD, $EF          ; MOV X, #$EF
    .byte $BD               ; MOV SP, X  
    .byte $8F, $AA, $F4     ; MOV $F4, #$AA
    .byte $8F, $BB, $F5     ; MOV $F5, #$BB
    .byte $2F, $FE          ; BRA *-2
    .byte $00, $00, $00, $00, $00, $00
