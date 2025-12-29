# Build script for PC workbench
# Assembles source.asm and creates/updates temp.img with the result

param(
    [string]$NasmPath = "",
    [string]$SourceAsm = "source.asm",
    [string]$OutputCom = "TEST.COM",
    [string]$TempImage = "images\temp.img"
)

# Resolve paths relative to script location
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SourceAsm = Join-Path $ScriptDir $SourceAsm
$OutputCom = Join-Path $ScriptDir $OutputCom
$TempImage = Join-Path $ScriptDir $TempImage

# Find NASM executable
if ([string]::IsNullOrEmpty($NasmPath)) {
    # Try global PATH first (cross-platform)
    $nasmCmd = Get-Command nasm -ErrorAction SilentlyContinue
    if ($nasmCmd) {
        $NasmPath = $nasmCmd.Source
        Write-Host "Using NASM from PATH: $NasmPath" -ForegroundColor Gray
    } else {
        # Try Windows-specific path
        $nasmCmd = Get-Command nasm.exe -ErrorAction SilentlyContinue
        if ($nasmCmd) {
            $NasmPath = $nasmCmd.Source
            Write-Host "Using NASM from PATH: $NasmPath" -ForegroundColor Gray
        } else {
            # Fallback to user profile location (Windows)
            $NasmPath = "$env:USERPROFILE\AppData\Local\bin\NASM\nasm.exe"
            if (-not (Test-Path $NasmPath)) {
                Write-Error "NASM not found. Install NASM or specify -NasmPath parameter."
                exit 1
            }
            Write-Host "Using NASM from user profile: $NasmPath" -ForegroundColor Gray
        }
    }
}

# Step 1: Assemble source.asm to TEST.COM
Write-Host "=== Assembling $SourceAsm ===" -ForegroundColor Cyan
& $NasmPath -f bin $SourceAsm -o $OutputCom
if ($LASTEXITCODE -ne 0) {
    Write-Error "Assembly failed!"
    exit 1
}
Write-Host "OK Assembled to $OutputCom" -ForegroundColor Green

# Step 2: Create blank FAT12 image if it doesn't exist
if (-not (Test-Path $TempImage)) {
    Write-Host "=== Creating blank FAT12 image ===" -ForegroundColor Cyan
    
    # Create 1.44MB blank image
    $imgSize = 1474560
    $blank = New-Object byte[] $imgSize
    
    # FAT12 boot sector (minimal, non-bootable)
    $bootSector = @(
        0xEB, 0x3C, 0x90,
        0x4D, 0x53, 0x44, 0x4F, 0x53, 0x35, 0x2E, 0x30,
        0x00, 0x02,
        0x01,
        0x01, 0x00,
        0x02,
        0xE0, 0x00,
        0x40, 0x0B,
        0xF0,
        0x09, 0x00,
        0x12, 0x00,
        0x02, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00
    )
    
    # Copy boot sector
    for ($i = 0; $i -lt $bootSector.Length; $i++) {
        $blank[$i] = $bootSector[$i]
    }
    
    # Boot signature
    $blank[510] = 0x55
    $blank[511] = 0xAA
    
    # Initialize FAT tables with media descriptor
    $fat1Offset = 512
    $blank[$fat1Offset] = 0xF0
    $blank[$fat1Offset + 1] = 0xFF
    $blank[$fat1Offset + 2] = 0xFF
    
    $fat2Offset = $fat1Offset + (9 * 512)
    $blank[$fat2Offset] = 0xF0
    $blank[$fat2Offset + 1] = 0xFF
    $blank[$fat2Offset + 2] = 0xFF
    
    # Write to file
    [System.IO.File]::WriteAllBytes($TempImage, $blank)
    Write-Host "OK Created $TempImage (1.44MB blank FAT12)" -ForegroundColor Green
}

# Step 3: Inject TEST.COM into temp.img
Write-Host "=== Injecting $OutputCom into $TempImage ===" -ForegroundColor Cyan

# Read image and COM file
$img = [System.IO.File]::ReadAllBytes($TempImage)
$com = [System.IO.File]::ReadAllBytes($OutputCom)

# Parse FAT12 boot sector
$bytesPerSector = [BitConverter]::ToUInt16($img, 0x0B)
$sectorsPerCluster = $img[0x0D]
$reservedSectors = [BitConverter]::ToUInt16($img, 0x0E)
$numFATs = $img[0x10]
$rootEntries = [BitConverter]::ToUInt16($img, 0x11)
$totalSectors = [BitConverter]::ToUInt16($img, 0x13)
$sectorsPerFAT = [BitConverter]::ToUInt16($img, 0x16)

# Calculate offsets
$fat1Offset = $reservedSectors * $bytesPerSector
$rootDirOffset = $fat1Offset + ($numFATs * $sectorsPerFAT * $bytesPerSector)
$dataOffset = $rootDirOffset + (($rootEntries * 32) / $bytesPerSector) * $bytesPerSector

# Calculate maximum cluster number based on disk geometry
$dataSectors = $totalSectors - ($dataOffset / $bytesPerSector)
$maxCluster = [int]($dataSectors / $sectorsPerCluster) + 1

# First, check if TEST.COM already exists and delete it
$existingEntry = -1
$existingCluster = -1
for ($i = 0; $i -lt $rootEntries; $i++) {
    $offset = $rootDirOffset + ($i * 32)
    $firstByte = $img[$offset]
    
    # Skip free/deleted entries
    if ($firstByte -eq 0x00 -or $firstByte -eq 0xE5) {
        continue
    }
    
    # Check if this is TEST.COM
    $entryName = ""
    for ($j = 0; $j -lt 11; $j++) {
        $entryName += [char]$img[$offset + $j]
    }
    
    if ($entryName -eq "TEST    COM") {
        $existingEntry = $offset
        $existingCluster = [BitConverter]::ToUInt16($img, $offset + 0x1A)
        Write-Host "Found existing TEST.COM at cluster $existingCluster, removing..." -ForegroundColor Yellow
        
        # Mark entry as deleted
        $img[$offset] = 0xE5
        
        # Free the cluster chain in FAT
        $cluster = $existingCluster
        while ($cluster -ge 2 -and $cluster -lt 0xFF8) {
            $fatOffset = $fat1Offset + [int]($cluster * 1.5)
            
            # Read next cluster in chain
            $nextCluster = 0
            if ($cluster % 2 -eq 0) {
                $nextCluster = [BitConverter]::ToUInt16($img, $fatOffset) -band 0xFFF
            } else {
                $nextCluster = ([BitConverter]::ToUInt16($img, $fatOffset) -shr 4) -band 0xFFF
            }
            
            # Mark cluster as free
            if ($cluster % 2 -eq 0) {
                $existing = [BitConverter]::ToUInt16($img, $fatOffset)
                $newValue = $existing -band 0xF000
                [BitConverter]::GetBytes([uint16]$newValue).CopyTo($img, $fatOffset)
            } else {
                $existing = [BitConverter]::ToUInt16($img, $fatOffset)
                $newValue = $existing -band 0x000F
                [BitConverter]::GetBytes([uint16]$newValue).CopyTo($img, $fatOffset)
            }
            
            # Move to next cluster or exit if EOF
            if ($nextCluster -ge 0xFF8) {
                break
            }
            $cluster = $nextCluster
        }
        
        break
    }
}

# Find first free entry in root directory
$entryOffset = -1
for ($i = 0; $i -lt $rootEntries; $i++) {
    $offset = $rootDirOffset + ($i * 32)
    $firstByte = $img[$offset]
    if ($firstByte -eq 0x00 -or $firstByte -eq 0xE5) {
        $entryOffset = $offset
        break
    }
}

if ($entryOffset -eq -1) {
    Write-Error "No free directory entries!"
    exit 1
}

# Find free cluster in FAT
$freeCluster = -1
for ($cluster = 2; $cluster -lt $maxCluster; $cluster++) {
    $fatOffset = $fat1Offset + [int]($cluster * 1.5)
    $fatValue = 0
    
    if ($cluster % 2 -eq 0) {
        $fatValue = [BitConverter]::ToUInt16($img, $fatOffset) -band 0xFFF
    } else {
        $fatValue = ([BitConverter]::ToUInt16($img, $fatOffset) -shr 4) -band 0xFFF
    }
    
    if ($fatValue -eq 0) {
        $freeCluster = $cluster
        break
    }
}

if ($freeCluster -eq -1) {
    Write-Error "No free clusters!"
    exit 1
}

# Create directory entry for TEST.COM
$fileName = "TEST    COM"
for ($i = 0; $i -lt 11; $i++) {
    $img[$entryOffset + $i] = [byte][char]$fileName[$i]
}

# Attributes (0x20 = archive)
$img[$entryOffset + 0x0B] = 0x20

# Time and date
$now = Get-Date
$time = (($now.Hour -shl 11) -bor ($now.Minute -shl 5) -bor ($now.Second / 2))
$date = ((($now.Year - 1980) -shl 9) -bor ($now.Month -shl 5) -bor $now.Day)
[BitConverter]::GetBytes([uint16]$time).CopyTo($img, $entryOffset + 0x16)
[BitConverter]::GetBytes([uint16]$date).CopyTo($img, $entryOffset + 0x18)

# Start cluster and file size
[BitConverter]::GetBytes([uint16]$freeCluster).CopyTo($img, $entryOffset + 0x1A)
[BitConverter]::GetBytes([uint32]$com.Length).CopyTo($img, $entryOffset + 0x1C)

# Write file data
$clusterOffset = $dataOffset + (($freeCluster - 2) * $sectorsPerCluster * $bytesPerSector)
[Array]::Copy($com, 0, $img, $clusterOffset, $com.Length)

# Mark cluster as EOF in FAT (0xFFF)
$fatOffset = $fat1Offset + [int]($freeCluster * 1.5)
if ($freeCluster % 2 -eq 0) {
    $existing = [BitConverter]::ToUInt16($img, $fatOffset)
    $newValue = ($existing -band 0xF000) -bor 0x0FFF
    [BitConverter]::GetBytes([uint16]$newValue).CopyTo($img, $fatOffset)
} else {
    $existing = [BitConverter]::ToUInt16($img, $fatOffset)
    $newValue = ($existing -band 0x000F) -bor (0x0FFF -shl 4)
    [BitConverter]::GetBytes([uint16]$newValue).CopyTo($img, $fatOffset)
}

# Copy to second FAT
$fat2Offset = $fat1Offset + ($sectorsPerFAT * $bytesPerSector)
for ($i = 0; $i -lt ($sectorsPerFAT * $bytesPerSector); $i++) {
    $img[$fat2Offset + $i] = $img[$fat1Offset + $i]
}

# Write image back
[System.IO.File]::WriteAllBytes($TempImage, $img)

Write-Host "OK Injected TEST.COM into $TempImage" -ForegroundColor Green
Write-Host ""
Write-Host "Ready to test - run the emulator and execute B:\TEST.COM in FreeDOS"
