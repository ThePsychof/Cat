import { createWriteStream, existsSync } from "node:fs";
import { mkdir, readFile, rm, copyFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import path from "node:path";
import { Readable } from "node:stream";
import { finished } from "node:stream/promises";

function resolveLocalAssetPath(assetFilename: string): string | undefined {
  const assetRoot = process.env.CAT_LOCAL_ASSET_ROOT;
  if (!assetRoot) return undefined;
  const candidate = path.join(assetRoot, assetFilename);
  return existsSync(candidate) ? candidate : undefined;
}

export async function downloadAsset(
  releaseTag: string,
  assetFilename: string,
  destDir: string,
  expectedSha256?: string,
  assetBaseUrlOverride?: string
): Promise<string> {
  const baseUrl = assetBaseUrlOverride ?? `https://github.com/ThePsychof/Cat/releases/download/${releaseTag}`;
  const url = `${baseUrl}/${assetFilename}`;

  if (!existsSync(destDir)) {
    await mkdir(destDir, { recursive: true });
  }
  const destPath = path.join(destDir, assetFilename);

  try {
    const res = await fetch(url);
    if (!res.ok || !res.body) {
      throw new Error(`Failed to download ${assetFilename}: ${res.status} ${res.statusText}`);
    }

    const nodeStream = Readable.fromWeb(res.body as any);
    const fileStream = createWriteStream(destPath);
    await finished(nodeStream.pipe(fileStream));

    if (expectedSha256) {
      const actual = createHash("sha256").update(await readFile(destPath)).digest("hex");
      if (actual.toLowerCase() !== expectedSha256.toLowerCase()) {
        await rm(destPath, { force: true });
        throw new Error(
          `Checksum mismatch for ${assetFilename}: expected ${expectedSha256}, got ${actual}. File deleted.`
        );
      }
    }

    return destPath;
  } catch {
    const localAssetPath = resolveLocalAssetPath(assetFilename);
    if (!localAssetPath) {
      throw new Error(`Failed to download ${assetFilename} from ${url}`);
    }
    await copyFile(localAssetPath, destPath);
    return destPath;
  }
}