# Hemulator Documentation Site

This directory contains the source files for the Hemulator documentation site, built with [Lumocs](https://lumocs.56k.guru/) and deployed to [https://hexagon.56k.guru](https://hexagon.56k.guru).

## Structure

```
docs/
├── src/                    # Documentation source files
│   ├── index.md           # Site homepage
│   ├── user/              # User documentation
│   │   └── manual.md      # Complete user manual
│   ├── developer/         # Developer documentation
│   │   ├── architecture.md   # Architecture overview (refers to root ARCHITECTURE.md)
│   │   ├── contributing.md   # Contributing guide
│   │   ├── n64-status.md     # N64 development status
│   │   ├── next-emulator.md  # Next system recommendation
│   │   └── sms-guide.md      # SMS implementation guide
│   ├── systems/           # System overview
│   │   └── index.md       # System documentation index
│   └── references/        # Technical references
│       ├── index.md       # Reference documentation index
│       ├── 6502.md        # 6502 CPU reference
│       ├── 65c816.md      # 65C816 CPU reference
│       ├── 8080.md        # 8080 CPU reference
│       ├── 8086.md        # 8086 CPU reference
│       ├── lr35902.md     # LR35902 CPU reference
│       ├── mips-r4300i.md # MIPS R4300i CPU reference
│       ├── spc700.md      # SPC700 CPU reference
│       ├── z80.md         # Z80 CPU reference
│       ├── pc-interrupts.md       # PC interrupt handling
│       └── spc700-ipl-protocol.md # SPC700 IPL protocol
├── _config.ts             # Lumocs configuration
├── deno.json              # Deno tasks and dependencies
├── .gitignore            # Excludes _site build output
├── ARCHITECTURE.md        # Redirect to site
├── CONTRIBUTING.md        # Redirect to site
├── MANUAL.md              # Redirect to site
├── N64_STATUS.md          # Redirect to site
├── NEXT_EMULATOR_RECOMMENDATION.md  # Redirect to site
├── SMS_IMPLEMENTATION_GUIDE.md      # Redirect to site
└── README.md              # This file
```

## Documentation Organization

### User Documentation (`src/user/`)
End-user guides, manuals, and getting started information. Accessible to non-technical users.

### Developer Documentation (`src/developer/`)
Architecture overview, contribution guidelines, implementation guides, and development resources.

**Note**: Full architecture documentation is maintained in the repository root at [ARCHITECTURE.md](https://github.com/Hexagon/hemulator/blob/master/ARCHITECTURE.md). The site provides a high-level overview.

### System Documentation (`src/systems/`)
Overview of all emulated systems with links to system-specific READMEs in `crates/systems/*/README.md`.

**Important**: System-specific implementation details stay in `crates/systems/*/README.md` to keep them close to the code. The site provides an overview and links to these READMEs.

### Reference Documentation (`src/references/`)
Technical references for CPUs, hardware components, and protocols. Includes instruction sets, addressing modes, and implementation notes with sources.

### Root Documentation Files (`docs/*.md`)
Most files in the root `docs/` directory are **simple redirects** for backward compatibility:
- `MANUAL.md`, `CONTRIBUTING.md`, `N64_STATUS.md`, `NEXT_EMULATOR_RECOMMENDATION.md`, `SMS_IMPLEMENTATION_GUIDE.md` → redirect to site
- `ARCHITECTURE.md` → redirects to repository root ARCHITECTURE.md
- `README.md` → this file, documenting the site itself

### Repository Root Documentation
- **[ARCHITECTURE.md](../ARCHITECTURE.md)** - Complete architecture documentation (repository root)
- **[AGENTS.md](../AGENTS.md)** - Implementation guidelines for automated agents and CI (repository root)

**The authoritative documentation is in `docs/src/`** and published to the site.

## Building the Documentation

### Prerequisites
- [Deno](https://deno.land/) 2.0.2 or later

### Local Development

**Serve with live reload**:
```bash
cd docs
deno task serve
```

Visit http://localhost:8000 to view the site.

**Build static site**:
```bash
cd docs
deno task build
```

Output will be in `docs/_site/`.

## Deployment

The documentation site is automatically deployed to GitHub Pages when changes are pushed to the `master` or `main` branch. The deployment workflow is defined in `.github/workflows/pages.yml`.

### Manual Deployment

To trigger a manual deployment:
1. Go to the Actions tab in GitHub
2. Select "Deploy Documentation with GitHub Pages"
3. Click "Run workflow"

## Adding New Documentation

### Adding User Documentation
1. Create a new `.md` file in `src/user/`
2. Add frontmatter with `title` and `nav_order`
3. Link from `src/index.md` or `src/user/manual.md`

### Adding Developer Documentation
1. Create a new `.md` file in `src/developer/`
2. Add frontmatter with `title` and `nav_order`
3. Link from `src/index.md` or `src/developer/architecture.md`

### Adding CPU/Hardware References
1. Create a new `.md` file in `src/references/`
2. Add frontmatter with `title`
3. Include references to datasheets and source materials
4. Link from `src/references/index.md`

### Frontmatter Format

All documentation files should include frontmatter:

```yaml
---
title: "Your Page Title"
nav_order: 1  # Optional: controls navigation order
---
```

## Maintaining Consistency

**Root Documentation Files**: Most files in the `docs/` directory are now **simple redirects** to the documentation site:
- `ARCHITECTURE.md`, `CONTRIBUTING.md`, `MANUAL.md`, `N64_STATUS.md`, `NEXT_EMULATOR_RECOMMENDATION.md`, `SMS_IMPLEMENTATION_GUIDE.md` → Redirect files
- `README.md` → Documentation about the site itself

**The authoritative source is `docs/src/`** - all documentation edits should be made there.

When updating documentation:
1. Edit files in `docs/src/` (this is the single source of truth)
2. Push to master/main to automatically deploy to the site
3. Do NOT edit the redirect files in `docs/` root
