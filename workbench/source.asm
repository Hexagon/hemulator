; Template for PC/DOS testing
; This file is assembled to TEST.COM and placed on B: drive (temp.img)
; Boot FreeDOS from A:, then run B:\TEST.COM to test your code

BITS 16
ORG 0x100

start:
    ; Print test message
    mov dx, msg_hello
    mov ah, 0x09            ; DOS: Print string
    int 0x21
    
    ; Exit to DOS
    mov ax, 0x4C00          ; DOS: Exit with return code 0
    int 0x21

; Data section
msg_hello:  db "Hello from TEST.COM on B: drive!", 13, 10, "$"
