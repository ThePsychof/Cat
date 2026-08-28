export type TargetOS = "windows" | "macos" | "linux";

export interface OSAssetEntry {
  launcher: string; // asset filename for the launcher binary/archive
}

export interface CatManifest {
  version: string;
  releaseTag: string;
  assets: Record<TargetOS, OSAssetEntry>;
}

const MANIFEST_URL =
  "https://raw.githubusercontent.com/ThePsychof/Cat/main/manifest.json";

export async function fetchManifest(): Promise<CatManifest> {
  const res = await fetch(MANIFEST_URL);
  if (!res.ok) {
    throw new Error(
      `Failed to fetch Cat manifest: ${res.status} ${res.statusText}`
    );
  }
  return (await res.json()) as CatManifest;
}