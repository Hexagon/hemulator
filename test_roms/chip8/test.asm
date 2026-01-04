; CHIP-8 Test ROM
; A minimal test program that draws a simple pattern to verify the emulator works

; Clear screen
00E0

; Load sprite address for '0' (font is at 0x000, '0' starts at 0x00)
A000  ; Set I = 0x000 (address of '0' sprite)

; Draw '0' at position (10, 10)
600A  ; Set V0 = 10 (X position)
610A  ; Set V1 = 10 (Y position)
D015  ; Draw sprite at (V0, V1), height 5

; Draw '8' at position (20, 10)  
6014  ; Set V0 = 20 (X position)
610A  ; Set V1 = 10 (Y position)
A028  ; Set I = 0x028 (address of '8' sprite - 8 * 5 bytes)
D015  ; Draw sprite at (V0, V1), height 5

; Infinite loop
121E  ; Jump to address 0x21E (this instruction)
