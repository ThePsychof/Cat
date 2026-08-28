import { promises as fs } from "node:fs";
import path from "node:path";

interface RemoteManifest {
  version: string;
  releaseTag: string;
  assets: Record<string, { launcher: string }>;
}

const MANIFEST_URL =
  "https://raw.githubusercontent.com/ThePsychof/Cat/main/manifest.json";

const STAGING_DIR = "update-staging";

function currentOSKey(): string {
  if (process.platform === "win32") return "windows";
  if (process.platform === "darwin") return "macos";
  return "linux";
}

export async function checkForUpdate(currentVersion: string): Promise<RemoteManifest | null> {
  const res = await fetch(MANIFEST_URL);
  if (!res.ok) {
    throw new Error(`Failed to fetch update manifest: ${res.status} ${res.statusText}`);
  }
  const manifest = (await res.json()) as RemoteManifest;

  if (manifest.version === currentVersion) {
    return null; // already up to date
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

  const stagingPath = path.join(driveRoot, ".cat", STAGING_DIR);
  await fs.mkdir(stagingPath, { recursive: true });

  const destFile = path.join(stagingPath, entry.launcher);
  const buffer = Buffer.from(await res.arrayBuffer());
  await fs.writeFile(destFile, buffer);

  // Marker file the next launch's bootstrap check looks for.
  await fs.writeFile(
    path.join(stagingPath, "pending.json"),
    JSON.stringify({ version: manifest.version, file: entry.launcher }, null, 2),
    "utf-8"
  );

  return destFile;
}

export async function getPendingUpdate(
  driveRoot: string
): Promise<{ version: string; file: string } | null> {
  const pendingPath = path.join(driveRoot, ".cat", STAGING_DIR, "pending.json");
  try {
    const raw = await fs.readFile(pendingPath, "utf-8");
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export async function applyPendingUpdate(driveRoot: string): Promise<void> {
  const pending = await getPendingUpdate(driveRoot);
  if (!pending) return;

  const stagingPath = path.join(driveRoot, ".cat", STAGING_DIR);
  const stagedFile = path.join(stagingPath, pending.file);
  const destFile = path.join(driveRoot, pending.file);
  const backupFile = `${destFile}.old`;

  // A backup from a previous update apply may still be sitting here if it
  // couldn't be deleted last run (its process hadn't fully exited yet).
  // Safe to clean up now, a full relaunch later.
  try {
    await fs.unlink(backupFile);
  } catch {
    // fine if it doesn't exist
  }

  // destFile is the executable currently running this very code. Windows
  // locks a running exe's content against being overwritten in place, but
  // still allows renaming it (only the directory entry changes) — so we
  // rename the old one out of the way, then move the staged build in.
  try {
    await fs.rename(destFile, backupFile);
  } catch (err) {
    throw new Error(`Could not replace running executable: ${(err as Error).message}`);
  }

  await fs.copyFile(stagedFile, destFile);

  await fs.unlink(stagedFile);
  await fs.unlink(path.join(stagingPath, "pending.json"));
}