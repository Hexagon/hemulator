# PC Emulator UI Workflow - Changes Summary

## Overview

This PR improves the PC emulator UI workflow by fixing Save/Open Project functionality and ensuring mount points are properly displayed.

## Visual Changes

### 1. Property Panel - Mount Points

**Before**: When creating a new PC system, the Mount Points section was empty.

**After**: Creating a new PC system immediately shows all mount points:

```
📊 Machine Metrics
  [collapsed section]

⚙️ Project Settings
  [collapsed section]

💿 Mount Points
  BIOS:        [Mount...] 
  Floppy A:    [Mount...]
  Floppy B:    [Mount...]
  Hard Drive:  [Mount...]
```

**After mounting a disk**:
```
💿 Mount Points
  BIOS:        [Mount...] 
  Floppy A:    x86boot.img [Eject]
  Floppy B:    [Mount...]
  Hard Drive:  [Mount...]
```

### 2. File Menu - Save Project

**Before**: 
- Click "File → Save Project..."
- Dialog opens at random location
- Navigate to desired folder
- Enter filename
- Save
- Next time: repeat full navigation

**After**:
- First save: Click "File → Save Project..."
  - Dialog opens at default location
  - Enter filename (e.g., "my_pc.hemu")
  - Save
- Second save: Click "File → Save Project..."
  - Dialog automatically opens to the folder containing "my_pc.hemu"
  - Filename pre-filled with "my_pc.hemu"
  - Just click Save (or change name if desired)

### 3. File Menu - Open Project

**Before**:
- Click "File → Open Project..."
- Load project
- (Project path not tracked internally)

**After**:
- Click "File → Open Project..."
- Load project
- Project path tracked internally
- Next "Save Project" defaults to this location

## Workflow Examples

### Example 1: Quick PC Testing

1. Launch emulator
2. Click "File → New Project..."
3. Select "PC"
4. **Immediately see mount points in Property Panel**
5. Click "Mount..." next to "Floppy A"
6. Select boot.img
7. PC boots from floppy

### Example 2: DOS Development

**Initial Setup**:
1. Create new PC system
2. Mount FreeDOS boot disk to Floppy A
3. Mount work disk to Floppy B
4. Click "File → Save Project..."
5. Save as "dos_dev.hemu"

**Daily Workflow**:
1. Launch emulator
2. Click "File → Recent Files → dos_dev.hemu"
   - All disks automatically mounted
3. Work in DOS
4. To save configuration changes:
   - Click "File → Save Project..."
   - Dialog defaults to "dos_dev.hemu"
   - Click Save (no navigation needed)

### Example 3: Managing Multiple Configurations

**Project 1**: DOS Boot Disk
```
dos_boot.hemu
  - Floppy A: freedos.img
  - Video: CGA
  - CPU: 8086
```

**Project 2**: Hard Drive System
```
dos_hdd.hemu
  - Hard Drive: hdd.img
  - Video: VGA
  - CPU: 80286
  - Boot: HardDriveFirst
```

Save each configuration once, then quickly switch between them via Recent Files menu.

## Technical Implementation

### Save Project Path Tracking

```rust
// RuntimeState tracks current project
struct RuntimeState {
    current_project_path: Option<PathBuf>,
    current_mounts: HashMap<String, String>,
    input_override: Option<settings::InputConfig>,
}

// After loading project
runtime_state.set_project_path(path.clone());

// After saving project
runtime_state.set_project_path(PathBuf::from(&saved_path));

// When saving again
save_project(&sys, &runtime_state, &settings, &mut msg,
    runtime_state.current_project_path.as_ref())  // Pass current path
```

### Mount Points for PC Systems

```rust
// Update mount points from current system
let is_pc_system = matches!(sys, EmulatorSystem::PC(_));
if rom_loaded || is_pc_system {
    // Populate mount points
    egui_app.property_pane.mount_points = mount_points_info
        .iter()
        .map(|mp| MountPoint { ... })
        .collect();
} else {
    egui_app.property_pane.mount_points.clear();
}
```

## User Benefits

1. **Faster workflow**: Save project defaults to last location
2. **Better discoverability**: Mount points visible immediately
3. **Less navigation**: One-click re-save to same project file
4. **Clearer intent**: UI shows all available mount points upfront
5. **Better documentation**: Comprehensive guide to PC GUI features

## Future Enhancements

- Add "File → Save" (without "...") for quick-save to current project
- Add "File → Save As..." to save to new location
- Show current project name in status bar or window title
- Auto-save project on mount/unmount actions
- Add "unsaved changes" indicator
