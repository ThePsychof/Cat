# Cat architecture plan

## Product intent

Cat is a portable Git repository vault that keeps repositories, metadata, and GitHub relationship information on a removable drive. The host machine is only a temporary runtime environment, not the source of truth.

The user experience is intentionally narrow and focused:

- browse repositories in a GitHub-like interface
- inspect branch and commit state
- compare local and remote repository state
- pull, push, fetch, and synchronize
- open a repository in an external editor without forcing a clone
- keep all Cat-specific app data on the Cat drive itself

## Core principles

1. The drive is Cat's home. The PC is temporary.
2. Cat must be self-contained on the drive.
3. The user should not need an installation step on each host machine.
4. The UI is a separate layer from the native Git and filesystem engine.
5. Repositories remain standard Git repositories on disk.
6. The orange cat is a guiding personality layer, not a distraction.

## Rust-first architecture

The codebase is evolving from a JavaScript/Electron prototype into a Rust-based native core.

### Layer model

```text
Cat application
├── UI layer
│   ├── repository browser
│   ├── branch and commit views
│   ├── sync and comparison panels
│   └── external editor launch controls
├── Rust core
│   ├── drive environment handling
│   ├── repository inspection
│   ├── Git operations
│   ├── filesystem state and metadata
│   ├── comparison logic
│   └── native process launching
└── portable drive storage
    ├── repositories/
    ├── .cat/
    ├── profiles/
    └── logs/
```

### Responsibilities

#### UI layer

- render the GitHub-inspired repository browser
- present repository status, diffs, and branch history
- handle actions like Pull, Push, Sync, Open in VS Code, Clone
- surface readable error states while preserving Cat's personality

#### Rust core

- manage the portable drive root and Cat metadata
- inspect repositories and their Git state
- execute Git operations using local/native tooling
- coordinate fetch, pull, push, clone, and comparison flows
- launch external editors and system commands in a controlled way
- keep host-side state minimal and ephemeral

## Compatibility with the current project

The existing TypeScript implementation already established useful concepts:

- drive provisioning with a portable environment
- a hidden `.cat` metadata directory
- Git operations around repository state
- a portable repository-oriented UI concept

The Rust migration preserves those concepts while replacing the brittle runtime coupling with a cleaner native model. The TypeScript workspace remains useful as a compatibility layer during the transition, but it is no longer the long-term architecture.

## Implementation guidelines

- Keep repository storage standard Git directories.
- Store Cat metadata under hidden drive-local directories.
- Let the UI query the core through a structured API instead of direct Git logic.
- Keep destructive operations explicit: Pull, Push, Sync, Clone, and Open in Editor should be obvious in the UI.
- Treat the drive as the durable state, not the host machine.
- Ensure the application remains portable, lightweight, and self-contained.

## Next milestone

The next practical step is to establish a Rust workspace and define a portable core API that the UI can consume. This repository now includes that foundation.
