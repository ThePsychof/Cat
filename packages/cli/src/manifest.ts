import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

export type TargetOS = "windows" | "macos" | "linux";

export interface OSAssetEntry {
  launcher: string; // asset filename for the launcher binary/archive
  icon: string;      // asset filename for the drive icon (.ico), used by autorun.inf
  launcherSha256: string; // hex sha256 of the launcher asset
  iconSha256: string;     // hex sha256 of the icon asset
}

export interface CatManifest {
  version: string;
  releaseTag: string;
  assets: Record<TargetOS, OSAssetEntry>;
}

const MANIFEST_URL =
  "https://raw.githubusercontent.com/ThePsychof/Cat/main/manifest.json";

function resolveManifestUrl(): string {
  return process.env.CAT_MANIFEST_URL ?? MANIFEST_URL;
}

function isLocalFileReference(value: string): boolean {
  return value.startsWith("file://") || value.startsWith("/") || /^[a-zA-Z]:[\\/]/.test(value);
}

async function readJsonFromPath(filePath: string): Promise<CatManifest> {
  const source = await readFile(filePath, "utf-8");
  return JSON.parse(source) as CatManifest;
}

export async function fetchManifest(manifestUrlOverride?: string): Promise<CatManifest> {
  const manifestUrl = manifestUrlOverride ?? resolveManifestUrl();

  if (isLocalFileReference(manifestUrl)) {
    const localPath = manifestUrl.startsWith("file://")
      ? fileURLToPath(manifestUrl)
      : manifestUrl;
    return readJsonFromPath(path.resolve(localPath));
  }

  try {
    const res = await fetch(manifestUrl);
    if (!res.ok) {
      throw new Error(`Failed to fetch Cat manifest: ${res.status} ${res.statusText}`);
    }
    return (await res.json()) as CatManifest;
  } catch {
    const assetRoot = process.env.CAT_LOCAL_ASSET_ROOT;
    if (!assetRoot) {
      throw new Error(`Failed to fetch Cat manifest from ${manifestUrl}`);
    }
    return readJsonFromPath(path.join(assetRoot, "manifest.json"));
  }
}