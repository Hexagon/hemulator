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
│   │   ├── architecture.md      # Architecture overview (refers to root ARCHITECTURE.md)
│   │   ├── contributing.md      # Contributing guide
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
├── ARCHITECTURE.md              # Redirect to repository root
├── CONTRIBUTING.md              # Redirect to site (backward compatibility)
├── MANUAL.md                    # Redirect to site (backward compatibility)
├── N64_STATUS.md                # Redirect to site (backward compatibility)
├── NEXT_EMULATOR_RECOMMENDATION.md  # Redirect to site (backward compatibility)
└── SMS_IMPLEMENTATION_GUIDE.md  # Redirect to site (backward compatibility)
```

### Repository Root Documentation

Key documentation files are maintained in the repository root:
- **ARCHITECTURE.md** - Complete architecture documentation
- **AGENTS.md** - Implementation guidelines for automated agents and CI
- **README.md** - Main repository README

### GitHub Pages Deployment

Created `.github/workflows/pages.yml` to automatically build and deploy the documentation site when changes are pushed to `master` or `main` branch.

**Site URL**: https://hexagon.56k.guru

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

### Changed Files

- **Root documentation** (`docs/*.md`): Converted to simple redirects
  - `ARCHITECTURE.md`, `CONTRIBUTING.md`, `MANUAL.md`, `N64_STATUS.md`, `NEXT_EMULATOR_RECOMMENDATION.md`, `SMS_IMPLEMENTATION_GUIDE.md` now contain links to the documentation site
  - Eliminates content duplication while maintaining backward compatibility
  - `docs/README.md` kept as guide for the documentation site itself
  
- **System READMEs** (`crates/systems/*/README.md`): Remain unchanged
  - System-specific implementation details stay close to the code
  - The documentation site links to these READMEs rather than duplicating them

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
- **Duplication Removal**: Root docs now redirect to site instead of duplicating content

### What We Added

- **Documentation site**: Lumocs-based static site
- **GitHub Pages workflow**: Automatic deployment
- **Site README**: Comprehensive guide for maintainers
- **Index pages**: Overview pages for each section
- **External references**: Links to datasheets and resources

## Backward Compatibility

### Direct Access Works as Redirects

Users can still access documentation files directly in the repository:
- `docs/MANUAL.md` - Now redirects to documentation site
- `docs/ARCHITECTURE.md` - Now redirects to documentation site
- `docs/CONTRIBUTING.md` - Now redirects to documentation site
- `docs/N64_STATUS.md` - Now redirects to documentation site
- `docs/NEXT_EMULATOR_RECOMMENDATION.md` - Now redirects to documentation site
- `docs/SMS_IMPLEMENTATION_GUIDE.md` - Now redirects to documentation site
- System READMEs unchanged - full content maintained

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

1. Edit the file in `docs/src/` (this is the authoritative source)
2. Verify links are correct
3. Push to master/main to deploy

**Note**: Root docs in `docs/*.md` (except `README.md`) are now redirects and should not be edited.

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
**Site URL**: https://hexagon.56k.guru  
**Repository**: https://github.com/Hexagon/hemulator
