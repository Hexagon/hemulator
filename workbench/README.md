# PC/DOS Testing Workbench

This directory provides a streamlined workflow for testing PC/DOS code in the emulator.

## Setup

The workbench uses a two-floppy configuration:

- **A: drive** (`x86boot.img`): FreeDOS boot disk - provides the operating system
- **B: drive** (`temp.img`): Test disk - contains only your compiled test program

## Quick Start

1. **Edit your code**: Modify `source.asm` with your test code

2. **Build and inject**: Run the build script

   ```powershell
   .\build.ps1
   ```
   This will:
   - Assemble `source.asm` to `TEST.COM`
   - Create `images\temp.img` if it doesn't exist
   - Inject `TEST.COM` into `temp.img`

3. **Run the emulator**:
   
   From the repo root:

   ```powershell
   cargo run --release -- workbench\workbench.hemu
   ```

4. **Test your code**: In FreeDOS, run:
   
   ```
   B:\TEST.COM
   ```

## Directory Structure

```
workbench/
├── workbench.hemu      # Emulator configuration (mounts A: and B:)
├── source.asm          # Your test code (edit this)
├── build.ps1           # Build script (assembles and injects)
├── TEST.COM            # Compiled output (generated)
├── README.md           # This file
└── images/
    ├── x86boot.img     # FreeDOS boot disk (A: drive)
    └── temp.img        # Test disk (B: drive, auto-created)
```

## Build Script Options

The build script accepts optional parameters:

```powershell
.\build.ps1 -NasmPath "path\to\nasm.exe" -SourceAsm "mycode.asm" -OutputCom "MYTEST.COM"
```

- **NasmPath**: Path to NASM assembler (default: auto-detected)
- **SourceAsm**: Source file to assemble (default: `source.asm`)
- **OutputCom**: Output COM file name (default: `TEST.COM`)
- **TempImage**: Output disk image (default: `images\temp.img`)

## Workflow Tips

### Testing INT 21h File I/O

When testing file operations, create test files on the B: drive:

```assembly
; Open file from B: drive
mov ax, 0x3D00          ; Open file, read-only
mov dx, testfile
int 0x21

testfile: db "B:\TESTDATA.TXT", 0
```

You can manually add files to `temp.img` using the inject script:
```powershell
..\inject_com.ps1 -DiskImage images\temp.img -ComFile mydata.txt -TargetFilename "TESTDATATXT"
```

### Iterative Testing

For rapid iteration:
1. Edit `source.asm`
2. Run `.\build.ps1` (rebuilds and injects)
3. In the running emulator, type `B:\TEST.COM` again
4. Repeat

The FreeDOS environment stays running - no need to restart unless you change the disk images.

### Debugging

Enable CPU tracing for detailed execution logs:
```powershell
cargo run --release -- --log-cpu trace workbench\workbench.hemu 2>&1 | Out-File trace.txt
```

Then analyze `trace.txt` to see exact instruction execution.

## Initial Setup (First Time Only)

Copy the FreeDOS boot disk to the workbench:
```powershell
Copy-Item test_roms\pc\x86BOOT.img workbench\images\x86boot.img
```

The first time you run `build.ps1`, it will automatically create `temp.img`.

## Example: Testing a File Read Loop

```assembly
BITS 16
ORG 0x100

start:
    ; Open file
    mov ax, 0x3D00
    mov dx, filename
    int 0x21
    jc error
    mov [handle], ax
    
read_loop:
    ; Read 256 bytes
    mov ah, 0x3F
    mov bx, [handle]
    mov cx, 256
    mov dx, buffer
    int 0x21
    
    ; Check for EOF
    test ax, ax
    jz eof
    
    ; Process data...
    jmp read_loop
    
eof:
    mov ah, 0x3E
    mov bx, [handle]
    int 0x21
    
    mov ax, 0x4C00
    int 0x21

error:
    mov ax, 0x4C01
    int 0x21

filename: db "B:\TESTDATA.TXT", 0
handle: dw 0
buffer: times 256 db 0
```

Build and test:
```powershell
.\build.ps1
cargo run --release -- workbench\workbench.hemu
# In FreeDOS: B:\TEST.COM
```
