---
title: "Save States"
parent: "User Manual"
nav_order: 4
---

# Save States

### Save States

Save states are stored in `saves/<rom_hash>/states.json`:
- Each game gets its own directory based on ROM hash
- 5 slots available per game
- **F5** opens the save slot selector - press 1-5 to select a slot (only for console systems)
- **F6** opens the load slot selector - press 1-5 to select a slot (shows which slots have saves)
- States are portable and can be backed up or transferred between systems
- **Important**: Save states do NOT include ROM/cartridge data - they only save emulator state
- The emulator verifies that the correct ROM is loaded before allowing state load
- If you try to load a state with a different ROM mounted, you'll get an error

**Save State Support by System**:
- **NES**: Fully supported - save and load states with F5-F6 when a cartridge is loaded
- **Atari 2600**: Fully supported - save and load states with F5-F6
- **Game Boy**: Fully supported - save and load states with F5-F6
- **PC/DOS**: Not supported - PC systems use **Project files** (.hemu) instead
  - **F8** saves the current VM configuration to a `.hemu` project file
  - **F7** loads a `.hemu` project file to restore all settings
  - VM files include all mounted disk images, BIOS, and boot priority settings
  - Disk state is preserved in the disk image files themselves (as in a real PC)
  - This approach matches how real PCs work - state persists on disks, not in memory snapshots

Example structure:
```
saves/
  ├── a1b2c3d4.../  (ROM hash)
  │   └── states.json
  └── e5f6g7h8.../
      └── states.json
```

