import { createWriteStream, existsSync } from "node:fs";
import { mkdir } from "node:fs/promises";
import path from "node:path";
import { Readable } from "node:stream";
import { finished } from "node:stream/promises";

export async function downloadAsset(
  releaseTag: string,
  assetFilename: string,
  destDir: string
): Promise<string> {
  const url = `https://github.com/ThePsychof/Cat/releases/download/${releaseTag}/${assetFilename}`;
  const res = await fetch(url);
  if (!res.ok || !res.body) {
    throw new Error(`Failed to download ${assetFilename}: ${res.status} ${res.statusText}`);
  }

  if (!existsSync(destDir)) {
    await mkdir(destDir, { recursive: true });
  }
  const destPath = path.join(destDir, assetFilename);

  const nodeStream = Readable.fromWeb(res.body as any);
  const fileStream = createWriteStream(destPath);
  await finished(nodeStream.pipe(fileStream));

  return destPath;
}