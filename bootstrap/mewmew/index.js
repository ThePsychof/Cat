#!/usr/bin/env node
import { createWriteStream, existsSync } from "node:fs";
import { chmod, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { pipeline } from "node:stream/promises";
import { spawn } from "node:child_process";

const version = "0.0.4";
const platform = process.platform === "win32" ? "windows" : process.platform === "darwin" ? "macos" : "linux";
const asset = `mewmew-${platform}-${process.arch}${platform === "windows" ? ".exe" : ""}`;

async function downloadAsset(folder, name) {
  const destination = path.join(folder, name);
  if (existsSync(destination)) return destination;
  const response = await fetch(`https://github.com/ThePsychof/Cat/releases/download/v${version}/${name}`);
  if (!response.ok || !response.body) throw new Error(`Unable to download Rust asset ${name}: ${response.status}`);
  await pipeline(response.body, createWriteStream(destination));
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