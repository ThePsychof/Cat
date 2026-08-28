# Cat 🐈

A portable GitHub development environment. Instead of your repos living only on one PC or in the cloud, Cat turns a USB drive, portable SSD, or SD card into a self-contained dev environment: repos, git identity/profiles, and tools travel with the drive.

Plug Cat into any supported PC, open it, and your repos are there — clone, pull, push, switch between GitHub profiles — with nothing installed on the host machine. Unplug it, and Cat comes with you.

The name is a nod to GitHub's Octocat — Octocat just sits there. Cat is the portable one.

## How it works

1. **Provision a drive** with a single command from any machine with Node.js installed:
   ```
   npx mewmew init /path/to/drive
   ```
   You'll be asked which OS(es) the drive should support (Windows / macOS / Linux / All). This downloads the matching portable git binary and Cat launcher for each selected OS onto the drive — no other install step, and nothing is installed on the host machine itself.

2. **Use Cat** by double-clicking the launcher at the root of the drive (`Cat.exe`, `Cat.app`, or the Linux AppImage, depending on what you selected). Everything from here happens inside Cat's own GUI:
   - Clone repositories
   - Push / pull
   - Create and switch between GitHub profiles (separate identity + credentials per profile, isolated per repo)
   - Check for and apply Cat updates, pulled from this repo

## Project structure

This repo is a TypeScript npm workspaces monorepo:

- `packages/cli` — the `cat-cli init` provisioning tool (Node/TypeScript, published via npm)
- `packages/gui` — the Cat launcher application (Electron/TypeScript), packaged per OS via `electron-builder`

## Status

Early development — not yet ready for real use. See open issues / project board for progress.

## License

TBD