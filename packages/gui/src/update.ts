import { filesystem } from "@neutralinojs/lib";
import { joinPath, ensureDir } from "./neutralino-paths.js";

interface RemoteManifest {
  version: string;
  releaseTag: string;
  assets: Record<string, { launcher: string; icon: string }>;
}

const MANIFEST_URL = "https://raw.githubusercontent.com/ThePsychof/Cat/main/manifest.json";
const STAGING_DIR = ".cat/update-staging";

function currentOSKey(): "windows" | "macos" | "linux" {
  const platform = navigator.platform.toLowerCase();
  if (platform.includes("win")) return "windows";
  if (platform.includes("mac")) return "macos";
  return "linux";
}

export async function checkForUpdate(currentVersion: string): Promise<RemoteManifest | null> {
  const res = await fetch(MANIFEST_URL);
  if (!res.ok) {
    throw new Error(`Failed to fetch update manifest: ${res.status} ${res.statusText}`);
  }
  const manifest = (await res.json()) as RemoteManifest;

  if (manifest.version === currentVersion) {
    return null;
  }
  return manifest;
}

export async function downloadUpdate(
  driveRoot: string,
  manifest: RemoteManifest
): Promise<string> {
  const osKey = currentOSKey();
  const entry = manifest.assets[osKey];
  if (!entry) {
    throw new Error(`No update asset available for ${osKey}`);
  }

  const url = `https://github.com/ThePsychof/Cat/releases/download/${manifest.releaseTag}/${entry.launcher}`;
  const res = await fetch(url);
  if (!res.ok || !res.body) {
    throw new Error(`Failed to download update: ${res.status} ${res.statusText}`);
  }

  const stagingPath = joinPath(driveRoot, STAGING_DIR);
  await ensureDir(stagingPath);

  const destFile = joinPath(stagingPath, entry.launcher);
  const buffer = await res.arrayBuffer();
  await filesystem.writeBinaryFile(destFile, buffer);

  await filesystem.writeFile(
    joinPath(stagingPath, "pending.json"),
    JSON.stringify({ version: manifest.version, file: entry.launcher }, null, 2)
  );

  return destFile;
}

export async function getPendingUpdate(
  driveRoot: string
): Promise<{ version: string; file: string } | null> {
  const pendingPath = joinPath(driveRoot, STAGING_DIR, "pending.json");
  try {
    const raw = await filesystem.readFile(pendingPath);
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export async function applyPendingUpdate(driveRoot: string): Promise<void> {
  const pending = await getPendingUpdate(driveRoot);
  if (!pending) return;

  const stagingPath = joinPath(driveRoot, STAGING_DIR);
  const stagedFile = joinPath(stagingPath, pending.file);
  const destFile = joinPath(driveRoot, pending.file);
  const backupFile = `${destFile}.old`;

  try {
    await filesystem.remove(backupFile);
  } catch {
    // fine if it doesn't exist
  }

  try {
    await filesystem.move(destFile, backupFile);
  } catch (err) {
    throw new Error(`Could not replace running executable: ${(err as Error).message}`);
  }

  await filesystem.move(stagedFile, destFile);
  await filesystem.remove(joinPath(stagingPath, "pending.json"));
}