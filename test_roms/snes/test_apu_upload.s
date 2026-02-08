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
    
    ; Step 2: Set up entry point and port 1 BEFORE sending command
    ; (SPC700 reads these immediately after receiving $CC)
    ; Entry point: The IPL stores the entry point at $0000/$0001, so we can't
    ; put code there. Indices 1-15 go to $0001-$000F. We'll use $0002 as entry
    ; point (skipping index 1, starting code at index 2).
    lda #$02
    sta $2142               ; Entry point low byte = $02
    lda #$00
    sta $2143               ; Entry point high byte = $00
    lda #$01
    sta $2141               ; Non-zero = upload mode (vs. execute mode)
    
    ; Step 3: Send upload start command ($CC)
    lda #$CC
    sta $2140
    
    ; CRITICAL: Wait for SPC700 to echo $CC
    ; The SPC700 needs time to process $CC and echo it back
:   lda $2140
    cmp #$CC
    bne :-
    
    ; CRITICAL: Write 0 to port 0 to signal "ready for upload"
    ; The IPL ROM at $FFD6-$FFD8 waits for port 0 = 0 before entering upload loop
    lda #$00
    sta $2140
    
    ; Small delay to give SPC700 time to see the 0 and enter upload loop
    ldy #$0010
:   dey
    bne :-
    
    ; Step 4: Upload some bytes
    ; The IPL ROM now waits for non-zero index at $FFDA
    ldx #$0001              ; Start with index 1
upload_loop1:
    ; Wait for SPC700 to be ready (port 0 should match previous index or be 0)
    ; This ensures we don't write new data before SPC700 processed the previous byte
    .a8
    cpx #$0001              ; First iteration?
    beq upload_first_byte   ; Skip wait on first byte
    ; Wait for previous index echo
    dex                     ; Get previous index  
    txa
    inx                     ; Restore current index
:   cmp $2140               ; Wait for SPC700 to echo previous index
    bne :-
upload_first_byte:
    ; Send index to port 0
    txa
    sta $2140
    
    ; Send data byte to port 1
    lda test_data, x
    sta $2141
    
    ; Wait for SPC700 to echo the index (acknowledge receipt)
    .a8
:   txa                     ; Get index back into A for comparison
    cmp $2140
    bne :-
    
    inx
    cpx #$0010              ; Upload 15 bytes (indices 1-15)
    bne upload_loop1
    
    lda #$02
    sta $0101               ; Marker: completed first upload
    
    ; Step 5: Signal end of upload
    ; After uploading index $0F, SPC700 expects index $10 next
    ; To exit the upload loop and make SPC700 jump to the entry point,
    ; we need to write port 0 > current index ($10)
    ; Port 1 = $00 signals execute mode (not upload mode)
    ; Keep entry point ports $F6/$F7 unchanged (still $0200)
    
    lda #$00
    sta $2141               ; Port 1 = $00 (execute mode)
    lda #$FF                ; Large value to trigger exit
    sta $2140               ; Port 0 = $FF > $10, triggers exit to $FFEF
    
    ; SPC700 will now:
    ; 1. Exit the upload loop at $FFE9/$FFED
    ; 2. Read entry point from ports $F6/$F7 ($0200)
    ; 3. Read ports $F4/$F5 at $FFF3 (will read $FF/$00)
    ; 4. Check port $F5 at $FFF9 (it's $00, so don't loop)
    ; 5. Jump to entry point at $FFFB
    ; 6. Execute uploaded code which writes $BBAA to ports
    
    ; Mark that we signaled the upload end
    ; The real test is whether the uploaded code executes (marker $0103)
    lda #$03
    sta $0102               ; Marker: end-of-upload signaled
    
    ; ========================================
    ; SECOND UPLOAD: This is where real games often hang!
    ; ========================================
    
    ; Give SPC700 lots of time to execute the uploaded code
    ; The uploaded code writes $BBAA to ports and loops
    ; We need to wait for it to finish writing the ports
    ldx #$8000              ; Large delay
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
    .byte $00               ; Index 1: dummy byte (will be overwritten by entry point)
    .byte $CD, $EF          ; Index 2-3: MOV X, #$EF
    .byte $BD               ; Index 4: MOV SP, X  
    .byte $8F, $AA, $F4     ; Index 5-7: MOV $F4, #$AA
    .byte $8F, $BB, $F5     ; Index 8-10: MOV $F5, #$BB
    .byte $2F, $FE          ; Index 11-12: BRA *-2
    .byte $00, $00, $00     ; Index 13-15: padding
