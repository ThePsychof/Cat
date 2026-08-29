import { filesystem, os } from "@neutralinojs/lib";

export function joinPath(...parts: string[]): string {
  return parts
    .map((p) => p.replace(/\/+$/, "").replace(/^\/+/, ""))
    .filter(Boolean)
    .join("/");
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await filesystem.getStats(path);
    return true;
  } catch {
    return false;
  }
}

// Neutralino's createDirectory has no documented recursive option, so
// nested paths (e.g. ".cat/update-staging") need each segment created
// in order, skipping ones that already exist.
export async function ensureDir(path: string): Promise<void> {
  const segments = path.split("/").filter(Boolean);
  let current = "";
  for (const segment of segments) {
    current = current ? `${current}/${segment}` : segment;
    if (!(await pathExists(current))) {
      await filesystem.createDirectory(current);
    }
  }
}

export { filesystem, os };