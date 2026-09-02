#!/usr/bin/env node
import { createWriteStream, existsSync } from "node:fs";
import { chmod, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";

const version = "0.0.5";
const platform = process.platform === "win32" ? "windows" : process.platform === "darwin" ? "macos" : "linux";
const asset = `mewmew-${platform}-${process.arch}${platform === "windows" ? ".exe" : ""}`;

const ORANGE = "\x1b[38;5;208m";
const GREEN = "\x1b[38;5;114m";
const RED = "\x1b[38;5;203m";
const DIM = "\x1b[2m";
const RESET = "\x1b[0m";
const isTTY = Boolean(process.stdout.isTTY);

// A paw print walks along a track proportional to real download progress —
// not a fake decorative loop. Falls back to plain percentage lines when
// output isn't a real terminal (CI, piped output, redirected to a file).
class CatProgress {
  constructor(label) {
    this.label = label;
    this.trackWidth = 24;
    this.lastLoggedStep = -1;
  }

  update(loaded, total) {
    if (!isTTY) {
      if (total) {
        const pct = Math.floor((loaded / total) * 100);
        const step = Math.floor(pct / 10);
        if (step !== this.lastLoggedStep) {
          this.lastLoggedStep = step;
          console.log(`${this.label}: ${pct}%`);
        }
      }
      return;
    }

    const pct = total ? loaded / total : 0;
    const pawPos = Math.min(this.trackWidth - 1, Math.floor(pct * this.trackWidth));
    const track = "─".repeat(pawPos) + "🐾" + "─".repeat(Math.max(0, this.trackWidth - pawPos - 1));
    const pctLabel = total ? `${Math.floor(pct * 100)}%`.padStart(4) : "····";
    process.stdout.write(`\r${ORANGE}${this.label}${RESET} [${track}] ${DIM}${pctLabel}${RESET}`);
  }

  succeed(message) {
    if (isTTY) {
      process.stdout.write(`\r${GREEN}=^ᵕ..ᵕ^= ${message}${" ".repeat(20)}${RESET}\n`);
    } else {
      console.log(message);
    }
  }

  fail(message) {
    if (isTTY) {
      process.stdout.write(`\r${RED}=;-;= ${message}${" ".repeat(20)}${RESET}\n`);
    } else {
      console.error(message);
    }
  }
}

async function downloadAsset(folder, name) {
  const destination = path.join(folder, name);
  if (existsSync(destination)) return destination;

  const progress = new CatProgress(`Fetching ${name}`);
  const response = await fetch(`https://github.com/ThePsychof/Cat/releases/download/v${version}/${name}`);
  if (!response.ok || !response.body) {
    progress.fail(`Couldn't download ${name} (${response.status})`);
    throw new Error(`Unable to download Rust asset ${name}: ${response.status}`);
  }

  const total = Number(response.headers.get("content-length")) || 0;
  let loaded = 0;

  const fileStream = createWriteStream(destination);
  const reader = response.body.getReader();

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      loaded += value.length;
      fileStream.write(value);
      progress.update(loaded, total);
    }
    fileStream.end();
    await new Promise((resolve, reject) => {
      fileStream.on("finish", resolve);
      fileStream.on("error", reject);
    });
  } catch (err) {
    progress.fail(`Download interrupted: ${err.message}`);
    throw err;
  }

  progress.succeed(`${name} ready.`);
  if (process.platform !== "win32") await chmod(destination, 0o755);
  return destination;
}

async function binaryPath() {
  const local = process.env.MEWMEW_BINARY ?? path.join(process.env.MEWMEW_ASSET_ROOT ?? "", asset);
  if (local && existsSync(local)) return local;
  const folder = path.join(tmpdir(), "mewmew", version);
  await mkdir(folder, { recursive: true });
  const destination = path.join(folder, asset);
  await downloadAsset(folder, asset);
  return destination;
}

if (process.argv[2] !== "init") throw new Error("Usage: npx mewmew init [drive-path] [--mode format|update|append]");
const provisioner = await binaryPath();
const folder = path.dirname(provisioner);
const catName = platform === "windows" ? "Cat.exe" : "Cat";
const catBinary = process.env.CAT_BINARY ?? await downloadAsset(folder, catName);
const child = spawn(provisioner, [...process.argv.slice(2), "--cat-binary", catBinary], { stdio: "inherit" });
child.on("exit", code => { process.exitCode = code ?? 1; });