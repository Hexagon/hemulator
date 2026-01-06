# Documentation Site Migration Summary

## Overview

This document describes the migration of Hemulator's documentation to a Lumocs-based static site hosted on GitHub Pages.

## What Changed

### New Documentation Site Structure

Created a comprehensive documentation site using [Lumocs](https://lumocs.56k.guru/) (a documentation-focused static site generator built on Lume):

```
docs/
├── src/                          # Documentation source files
│   ├── index.md                 # Homepage
│   ├── user/                    # User documentation
│   │   └── manual.md            # Complete user manual
│   ├── developer/               # Developer documentation
│   │   ├── architecture.md      # System architecture
│   │   ├── contributing.md      # Contributing guide
│   │   ├── agents.md            # Agent guidelines
│   │   ├── n64-status.md        # N64 development status
│   │   ├── next-emulator.md     # Next system recommendation
│   │   └── sms-guide.md         # SMS implementation guide
│   ├── systems/                 # System overview
│   │   └── index.md             # System documentation index
│   └── references/              # Technical references
│       ├── index.md             # Reference index
│       ├── 6502.md              # 6502 CPU reference
│       ├── 65c816.md            # 65C816 CPU reference
│       ├── 8080.md              # 8080 CPU reference
│       ├── 8086.md              # 8086 CPU reference
│       ├── lr35902.md           # Game Boy CPU reference
│       ├── mips-r4300i.md       # N64 CPU reference
│       ├── spc700.md            # SNES audio CPU reference
│       ├── z80.md               # Z80 CPU reference
│       ├── pc-interrupts.md     # PC interrupt handling
│       └── spc700-ipl-protocol.md # SNES audio boot protocol
├── _config.ts                   # Lumocs configuration
├── deno.json                    # Deno tasks and dependencies
├── .gitignore                   # Excludes _site build output
├── README.md                    # Documentation site guide
├── ARCHITECTURE.md              # Architecture (root copy for backward compatibility)
├── CONTRIBUTING.md              # Contributing (root copy for backward compatibility)
└── MANUAL.md                    # User manual (root copy for backward compatibility)
```

### GitHub Pages Deployment

Created `.github/workflows/pages.yml` to automatically build and deploy the documentation site when changes are pushed to `master` or `main` branch.

**Site URL**: https://hexagon.github.io/hemulator/

### Updated Files

1. **AGENTS.md**: Added comprehensive documentation structure guidelines
   - Documentation organization explained
   - Lumocs workflow documented
   - Maintenance guidelines added

2. **README.md**: Updated to prominently feature the documentation site
   - Added documentation site link at the top of user/developer sections
   - Improved navigation structure
   - Clear distinction between user and developer resources

3. **docs/README.md**: Created comprehensive guide for documentation maintainers
   - Site structure explained
   - Building and deployment instructions
   - Adding new documentation guidelines
   - Maintenance best practices

### Unchanged Files

- **System READMEs** (`crates/systems/*/README.md`): Remain in place
  - System-specific implementation details stay close to the code
  - The documentation site links to these READMEs rather than duplicating them
  
- **Root documentation** (`docs/*.md`): Maintained for backward compatibility
  - `ARCHITECTURE.md`, `CONTRIBUTING.md`, `MANUAL.md` kept in docs/
  - Content mirrored in `docs/src/` for the site

## Key Features

### 1. Organized Structure

Documentation is now clearly organized into:
- **User Documentation**: Getting started, controls, features
- **Developer Documentation**: Architecture, contributing, implementation guides
- **System Documentation**: Overview with links to system-specific details
- **Reference Documentation**: CPU and hardware technical references

### 2. Comprehensive References

All reference documentation includes:
- Frontmatter with title and navigation order
- Links to external datasheets and resources
- Instruction sets and addressing modes
- Implementation notes

### 3. Automatic Deployment

GitHub Actions workflow automatically:
- Installs Deno
- Builds the Lumocs site
- Deploys to GitHub Pages
- Runs on push to master/main or manual trigger

### 4. Local Development Support

Developers can:
- Serve site locally: `cd docs && deno task serve`
- Build site locally: `cd docs && deno task build`
- View at http://localhost:8000

## Benefits

### For Users

1. **Better Navigation**: Clear structure with automatic navigation menu
2. **Search Functionality**: Full-text search across all documentation
3. **Responsive Design**: Works on mobile, tablet, and desktop
4. **Professional Presentation**: Clean, modern documentation site

### For Developers

1. **Organized References**: CPU and hardware references in one place
2. **Clear Guidelines**: Implementation patterns and best practices documented
3. **Easy Updates**: Markdown-based with simple frontmatter
4. **Version Control**: All documentation in git with full history

### For Contributors

1. **Clear Structure**: Easy to find where to add documentation
2. **Automatic Deployment**: Push to master/main to update site
3. **Local Testing**: Serve site locally before committing
4. **Consistent Format**: Frontmatter ensures consistent presentation

## Migration Philosophy

### What We Kept

- **System READMEs**: Implementation details stay in `crates/systems/*/README.md`
- **Root docs**: `docs/*.md` maintained for backward compatibility
- **References section**: All technical references tracked in system READMEs

### What We Changed

- **Organization**: Moved from flat structure to organized sections
- **Presentation**: Static site instead of raw markdown
- **Navigation**: Automatic navigation from frontmatter
- **Cross-references**: Updated to use proper site paths

### What We Added

- **Documentation site**: Lumocs-based static site
- **GitHub Pages workflow**: Automatic deployment
- **Site README**: Comprehensive guide for maintainers
- **Index pages**: Overview pages for each section
- **External references**: Links to datasheets and resources

## Backward Compatibility

### Direct Access Still Works

Users can still access documentation directly:
- `docs/MANUAL.md` - Still available in repository
- `docs/ARCHITECTURE.md` - Still available in repository
- `docs/CONTRIBUTING.md` - Still available in repository
- System READMEs unchanged

### Updated Links

Internal documentation links updated to:
- Use relative paths within the site
- Use absolute GitHub URLs for system READMEs
- Work correctly in both site and repository views

## Future Enhancements

Potential improvements for the documentation site:

1. **Search Optimization**: Configure advanced search features
2. **Code Examples**: Add more code snippets and examples
3. **Diagrams**: Add architecture diagrams and flowcharts
4. **API Documentation**: Auto-generate API docs from code
5. **Version Support**: Add version selector for different releases
6. **Translations**: Support for multiple languages
7. **Contribution Templates**: Add templates for new documentation

## Maintenance

### Adding New Documentation

1. Create `.md` file in appropriate `docs/src/` subdirectory
2. Add frontmatter with `title` and optional `nav_order`
3. Link from appropriate index page
4. Push to master/main to deploy

### Updating Existing Documentation

1. Edit the file in `docs/src/`
2. For root docs, also update corresponding file in `docs/`
3. Verify links are correct
4. Push to master/main to deploy

### Testing Changes

```bash
cd docs
deno task serve
# Visit http://localhost:8000
```

## Technical Details

### Lumocs Configuration

- **Lume Version**: 2.3.3
- **Lumocs Version**: 0.1.3
- **Deno Version**: 2.0.2
- **Source Directory**: `docs/src/`
- **Output Directory**: `docs/_site/` (excluded from git)

### GitHub Actions Workflow

- **Name**: Deploy Documentation with GitHub Pages
- **Triggers**: Push to master/main, manual workflow dispatch
- **Permissions**: Read contents, write pages, write id-token
- **Jobs**: Build (install Deno, run Lume), Deploy (deploy to Pages)

### Build Process

1. Checkout repository
2. Install Deno 2.0.2
3. Run `deno task lume` in `docs/`
4. Upload `docs/_site/` as Pages artifact
5. Deploy artifact to GitHub Pages

## Resources

- **Lumocs Documentation**: https://lumocs.56k.guru/
- **Lume Documentation**: https://lume.land/
- **Deno Documentation**: https://deno.land/
- **GitHub Pages**: https://docs.github.com/en/pages

## Summary

This migration transforms Hemulator's documentation from a collection of markdown files into a professional, organized, searchable documentation site while maintaining backward compatibility and keeping implementation details close to the code. The site automatically deploys via GitHub Actions and provides an excellent experience for both users and developers.

---

**Migration Date**: January 6, 2026  
**Site URL**: https://hexagon.github.io/hemulator/  
**Repository**: https://github.com/Hexagon/hemulator
