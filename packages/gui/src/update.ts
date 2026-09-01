import { filesystem, os } from "@neutralinojs/lib";
import { joinPath, ensureDir } from "./neutralino-paths.js";

interface UpdateAsset {
  launcher: string;
  icon: string;
  launcherSha256: string;
  iconSha256: string;
}

interface RemoteManifest {
  version: string;
  releaseTag: string;
  assets: Record<string, UpdateAsset>;
}

const MANIFEST_URL = "https://raw.githubusercontent.com/ThePsychof/Cat/main/manifest.json";
const STAGING_DIR = ".cat/update-staging";

function currentOSKey(): "windows" | "macos" | "linux" {
  if (NL_OS === "Windows") return "windows";
  if (NL_OS === "Darwin") return "macos";
  return "linux";
}

export async function checkForUpdate(currentVersion: string): Promise<RemoteManifest | null> {
  const res = await fetch(MANIFEST_URL);
  if (!res.ok) {
    throw new Error(`Failed to fetch update manifest: ${res.status} ${res.statusText}`);
  }
  const manifest = (await res.json()) as RemoteManifest;

  if (!isNewerVersion(manifest.version, currentVersion)) {
    return null;
  }
  return manifest;
}

function isNewerVersion(remote: string, current: string): boolean {
  const parse = (v: string) => v.split(".").map((n) => parseInt(n, 10) || 0);
  const [rMaj, rMin, rPatch] = parse(remote);
  const [cMaj, cMin, cPatch] = parse(current);
  if (rMaj !== cMaj) return rMaj > cMaj;
  if (rMin !== cMin) return rMin > cMin;
  return rPatch > cPatch;
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
  if (!entry.launcherSha256) {
    throw new Error(`Manifest missing launcherSha256 for ${osKey} — refusing to download unverifiable update.`);
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

  const digest = await crypto.subtle.digest("SHA-256", buffer);
  const actualSha = Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0")).join("");
  if (actualSha.toLowerCase() !== entry.launcherSha256.toLowerCase()) {
    throw new Error(`Update checksum mismatch for ${entry.launcher} — refusing to stage.`);
  }

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

  if (currentOSKey() !== "windows") {
    await os.execCommand(`chmod +x "${destFile}"`);
  }

  await filesystem.remove(joinPath(stagingPath, "pending.json"));
}