# Cat 🐈

Cat is a portable live Git repository backup and synchronization utility built to live on a flash drive. The drive is the persistent home of the repositories and Cat itself; the PC is only a temporary runtime environment.

The user plugs the drive into a computer, opens Cat, sees their repositories in a GitHub-inspired interface, and can inspect, compare, synchronize, clone, or open them in an external editor. Cat stays focused on the repository workflow instead of becoming an IDE, clone of GitHub, or general-purpose file manager.

## Product definition

Cat is the interface between three durable concepts:


The full experience is:


## The flash drive is Cat's home

Cat is intentionally designed around a zero-install host model:


The drive contains the Cat environment itself. The host merely runs it.

## Rust-first architecture

The long-term architecture is a Rust-native core with a separate UI layer:

```text
Cat application
├── UI layer
│   ├── repository browser
│   ├── branch and commit views
│   ├── compare / sync controls
│   └── external editor launch actions
├── Rust core
│   ├── drive environment handling
│   ├── repository inspection
│   ├── Git operations
│   ├── filesystem operations
│   ├── comparison and diff logic
│   └── native process launching
└── portable drive storage
    ├── repositories/
    ├── .cat/
    ├── profiles/
    └── logs/
```

This repo is in migration from the earlier TypeScript/Electron prototype toward that native design. The prototype established the portable-drive concept and the repository-first interaction model. The Rust workspace now preserves the useful ideas while laying down a cleaner and lighter architecture.

## Current project layout


## Product personality

Cat is intentionally:


It is not a full GitHub clone, an IDE, a file manager, or a social platform.

## Current status

The migration has begun. The Rust workspace and foundational models are in place, and the project is now organized around the portable-drive-first product definition described in the architecture document.

## Installation & User Workflow

### Creating a Cat Drive

The intended workflow is: a normal PC runs `npx mewmew init` against a USB drive, and that drive becomes a self-contained Cat environment. The host machine is only a temporary bootstrap environment; the flash drive is where Cat lives.

```bash
npx mewmew init /path/to/flash-drive
```

`npx` fetches the published `mewmew` CLI automatically, then prepares the selected drive. The CLI allows the user to choose a drive and a mode:

#### Format

Creates a fresh Cat drive from scratch. This is the cleanest path for a new portable Cat environment.

#### Update

Updates Cat on an existing Cat drive while preserving repositories, profiles, and user data.

#### Append

Adds Cat to an existing drive without destroying unrelated files already on that drive.

The complete flow is:

```text
Any PC
  ↓
  npx mewmew init /path/to/drive
  ↓
Mewmew downloads/runs automatically
  ↓
Select flash drive
  ↓
├── Format
├── Update
└── Append
  ↓
🐈 Portable Cat drive
```

A prepared drive may be either:

- fresh: a new, empty Cat environment
- full: an existing drive that already contains repositories, profiles, and Cat state

Either form is portable. Once the drive has been initialized, Cat is meant to live on it independently. The host PC does not become Cat's home.

There is no separate package-download step for the user. `mewmew` is public on npm and `npx` is the entry point. Once the drive has been prepared, `mewmew` is not required for normal Cat usage.

### Using the Cat Drive

After initialization, the Cat drive is portable. The user can unplug it from the preparation PC and use it on another compatible computer.

```text
Prepare once
  ↓
🐈 Cat Drive
  ↓
PC #1
  ↓
PC #2
  ↓
PC #3
  ↓
Public / Family / Other PC
```

The host computer should not need Node.js, npm, `mewmew`, Rust, Git, or any Cat installation just to run Cat. Cat runs from the prepared drive. The intended experience is:

```text
Plug in Cat
  ↓
Launch Cat
  ↓
Use Cat
  ↓
Close Cat
  ↓
Safely eject
```

The drive is Cat's persistent environment; the host PC is only a temporary place where Cat runs.

### Editing From the Drive

The user does not have to clone a repository to the PC before editing. VS Code or another editor can open the repository directly from the Cat drive.

```text
Cat drive
  ↓
Repository folder
  ↓
Open in VS Code
  ↓
Edit directly
  ↓
Commit
  ↓
Cat
  ↓
Push / Sync
```

Cloning to the PC is still available as an optional workflow:

```text
Cat drive
  ↓
Clone repository
  ↓
PC workspace
  ↓
VS Code / other editor
  ↓
Edit
  ↓
Commit
```

Cloning is optional.

### Host-PC Principle

Cat is designed to be used on computers that may not belong to the user. Cat should operate without intentionally installing itself or establishing unnecessary permanent configuration on the host.

Do not require:


Cat's application data, configuration, cache, and persistent state should live on the Cat drive whenever technically practical. The goal is:

```text
Plug in → run Cat → use Cat → eject the drive.
```

The OS may independently record USB insertion, process execution, filesystem activity, security events, etc. Cat should not claim forensic invisibility; the requirement is that Cat itself does not intentionally leave unnecessary persistent application data or installation artifacts on the host.

## License

GPL-3.0
