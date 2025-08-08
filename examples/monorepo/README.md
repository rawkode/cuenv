# Monorepo Example

This example demonstrates cuenv's discovery feature in a monorepo structure.

## Structure

```
monorepo/
├── cue.mod/          # CUE module root
│   └── module.cue
├── env.cue           # Root environment
├── projects/
│   ├── frontend/     # Frontend project
│   │   └── env.cue   # Frontend-specific environment
│   └── backend/      # Backend project
│       └── env.cue   # Backend-specific environment
└── tools/
    ├── ci/           # CI/CD tools
    │   └── env.cue   # CI environment variables
    └── scripts/      # Build scripts
        └── env.cue   # Script environment variables
```

## Usage

### Discover all packages

From anywhere in the monorepo:

```bash
cuenv discover
```

Expected output:

```
Discovered CUE packages:
  root -> /path/to/monorepo
  projects:frontend -> /path/to/monorepo/projects/frontend
  projects:backend -> /path/to/monorepo/projects/backend
  tools:ci -> /path/to/monorepo/tools/ci
  tools:scripts -> /path/to/monorepo/tools/scripts
```

### Discover and load packages

```bash
cuenv discover --load
```

This will also validate that each package can be successfully loaded.

### Load a specific environment

Navigate to any subdirectory and use cuenv normally:

```bash
cd projects/frontend
cuenv reload
```

Or from the root, you could potentially load a specific package (future feature):

```bash
cuenv load projects:frontend
```

## Package Naming Convention

The discovery feature uses a hierarchical naming convention:

- Root package: `root`
- Nested packages: `parent:child` (using colon as separator)
- Examples:
  - `projects:frontend`
  - `projects:backend`
  - `tools:ci`
  - `tools:scripts`

This makes it easy to identify and organize packages in large monorepos.
