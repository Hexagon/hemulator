# Hemulator Documentation Site

This directory contains the source files for the Hemulator documentation site, built with [Lumocs](https://lumocs.56k.guru/) and deployed to [https://hexagon.github.io/hemulator/](https://hexagon.github.io/hemulator/).

## Structure

```
docs/
├── src/                    # Documentation source files
│   ├── index.md           # Site homepage
│   ├── user/              # User documentation
│   │   └── manual.md      # Complete user manual
│   ├── developer/         # Developer documentation
│   │   ├── architecture.md   # System architecture
│   │   ├── contributing.md   # Contributing guide
│   │   ├── agents.md         # Agent guidelines
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
├── ARCHITECTURE.md        # Architecture (root copy)
├── CONTRIBUTING.md        # Contributing (root copy)
├── MANUAL.md              # User manual (root copy)
└── README.md              # This file
```

## Documentation Organization

### User Documentation (`src/user/`)
End-user guides, manuals, and getting started information. Accessible to non-technical users.

### Developer Documentation (`src/developer/`)
Architecture details, contribution guidelines, implementation guides, and development resources.

### System Documentation (`src/systems/`)
Overview of all emulated systems with links to system-specific READMEs in `crates/systems/*/README.md`.

**Important**: System-specific implementation details stay in `crates/systems/*/README.md` to keep them close to the code. The site provides an overview and links to these READMEs.

### Reference Documentation (`src/references/`)
Technical references for CPUs, hardware components, and protocols. Includes instruction sets, addressing modes, and implementation notes with sources.

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

**Root Documentation Files**: The `docs/` directory contains copies of key documentation files for backward compatibility and direct access:
- `ARCHITECTURE.md`
- `CONTRIBUTING.md`
- `MANUAL.md`

When updating these files, update both the root copy and the `src/` version:
1. Edit `docs/ARCHITECTURE.md`
2. Copy changes to `docs/src/developer/architecture.md` (preserving frontmatter)
3. Or vice versa, depending on workflow

**System READMEs**: Always keep system-specific implementation details in `crates/systems/*/README.md`. The site should link to these READMEs rather than duplicating their content.

## Links and Cross-References

Use relative links within the documentation site:
- User docs: `[Link](../user/manual.md)`
- Developer docs: `[Link](../developer/architecture.md)`
- References: `[Link](../references/6502.md)`

For GitHub links (system READMEs), use absolute URLs:
- `[NES README](https://github.com/Hexagon/hemulator/blob/master/crates/systems/nes/README.md)`

## Styling and Features

Lumocs provides:
- Automatic navigation from frontmatter
- Markdown rendering with GitHub Flavored Markdown
- Code syntax highlighting
- Automatic table of contents
- Search functionality
- Responsive design

See [Lumocs documentation](https://lumocs.56k.guru/) for advanced features.

## Troubleshooting

**Site not updating**: 
- Check GitHub Actions for build errors
- Verify Pages is enabled in repository settings
- Ensure `_site/` is in `.gitignore`

**Broken links**:
- Use relative links within the site
- Test locally with `deno task serve`
- Check navigation order in frontmatter

**Deno errors**:
- Ensure Deno 2.0.2+ is installed
- Run `deno task lume` to update dependencies
- Check `deno.json` for correct Lumocs version

## Resources

- [Lumocs Documentation](https://lumocs.56k.guru/)
- [Lume Static Site Generator](https://lume.land/)
- [GitHub Pages Documentation](https://docs.github.com/en/pages)
