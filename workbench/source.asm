; Test program to mimic FreeDOS 'type' command behavior
; Opens A:\FDAUTO.BAT, reads and prints it with debug output
; This helps debug the infinite loop issue without full CPU tracing

BITS 16
ORG 0x100

start:
    ; Print startup message
    mov dx, msg_start
    mov ah, 0x09
    int 0x21
    
    ; Open file A:\FDAUTO.BAT (read-only)
    mov ax, 0x3D00          ; AH=0x3D (Open file), AL=0x00 (read-only)
    mov dx, filename
    int 0x21
    jc open_error
    
    ; Save file handle
    mov [file_handle], ax
    
    ; Print file opened message
    mov dx, msg_opened
    mov ah, 0x09
    int 0x21

read_loop:
    ; Read chunk from file (512 bytes at a time)
    mov ah, 0x3F            ; DOS: Read from file
    mov bx, [file_handle]
    mov cx, 512             ; Read 512 bytes
    mov dx, buffer          ; Buffer offset
    int 0x21
    jc read_error
    
    ; Save bytes read count
    mov [bytes_read], ax
    
    ; Print debug: bytes read
    push ax
    mov dx, msg_read
    mov ah, 0x09
    int 0x21
    pop ax
    
    ; Print the count in hex
    call print_hex_word
    
    ; Print newline
    mov dx, msg_newline
    mov ah, 0x09
    int 0x21
    
    ; Check if we read any bytes (EOF check)
    cmp word [bytes_read], 0
    je eof_reached
    
    ; Print the data we read
    mov cx, [bytes_read]
    mov si, buffer
print_char_loop:
    lodsb                   ; Load byte from DS:SI into AL, increment SI
    
    ; Print character using INT 29h (fast console output)
    int 0x29
    
    loop print_char_loop
    
    ; Continue reading
    jmp read_loop

eof_reached:
    ; Print EOF message
    mov dx, msg_eof
    mov ah, 0x09
    int 0x21
    
    ; Close file
    mov ah, 0x3E
    mov bx, [file_handle]
    int 0x21
    
    ; Print success message
    mov dx, msg_success
    mov ah, 0x09
    int 0x21
    
    ; Exit normally
    mov ax, 0x4C00
    int 0x21

open_error:
    ; Print error opening file
    mov dx, msg_open_err
    mov ah, 0x09
    int 0x21
    
    ; Print error code
    push ax
    mov al, ah              ; Error code is in AH
    xor ah, ah
    call print_hex_word
    pop ax
    
    mov dx, msg_newline
    mov ah, 0x09
    int 0x21
    
    ; Exit with error
    mov ax, 0x4C01
    int 0x21

read_error:
    ; Print error reading file
    mov dx, msg_read_err
    mov ah, 0x09
    int 0x21
    
    ; Print error code
    push ax
    mov al, ah
    xor ah, ah
    call print_hex_word
    pop ax
    
    mov dx, msg_newline
    mov ah, 0x09
    int 0x21
    
    ; Close file
    mov ah, 0x3E
    mov bx, [file_handle]
    int 0x21
    
    ; Exit with error
    mov ax, 0x4C01
    int 0x21

; Print AX as 4-digit hex number
print_hex_word:
    push ax
    push bx
    push cx
    push dx
    
    mov bx, ax
    mov cx, 4               ; 4 hex digits
.digit_loop:
    rol bx, 4               ; Rotate left 4 bits to get next digit
    mov al, bl
    and al, 0x0F            ; Mask to get low nibble
    
    ; Convert to ASCII
    cmp al, 9
    jbe .is_digit
    add al, 'A' - 10
    jmp .print_it
.is_digit:
    add al, '0'
.print_it:
    ; Print using INT 29h
    int 0x29
    
    loop .digit_loop
    
    pop dx
    pop cx
    pop bx
    pop ax
    ret

; Data section
filename:       db "A:\FDAUTO.BAT", 0
file_handle:    dw 0
bytes_read:     dw 0

msg_start:      db "[DEBUG] Starting file read test...", 13, 10, "$"
msg_opened:     db "[DEBUG] File opened successfully", 13, 10, "$"
msg_read:       db "[DEBUG] Read bytes: $"
msg_eof:        db "[DEBUG] EOF reached", 13, 10, "$"
msg_success:    db "[DEBUG] File read complete, exiting normally", 13, 10, "$"
msg_open_err:   db "[ERROR] Failed to open file, error code: $"
msg_read_err:   db "[ERROR] Failed to read file, error code: $"
msg_newline:    db 13, 10, "$"

; Buffer for reading file data (512 bytes)
buffer:         times 512 db 0
