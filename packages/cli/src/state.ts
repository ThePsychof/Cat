import { promises as fs } from "node:fs";
import path from "node:path";
import { STATE_FILENAME, type CatState } from "@cat/shared";

export type { RepoEntry, GitProfile, CatState } from "@cat/shared";

function statePath(driveRoot: string): string {
  return path.join(driveRoot, ".cat", STATE_FILENAME);
}

export async function readState(driveRoot: string): Promise<CatState> {
  const p = statePath(driveRoot);
  try {
    const raw = await fs.readFile(p, "utf-8");
    return JSON.parse(raw) as CatState;
  } catch {
    return { version: "0.0.1", selectedOS: [], activeProfile: null, profiles: [], repos: [] };
  }
}

export async function writeState(driveRoot: string, state: CatState): Promise<void> {
  const p = statePath(driveRoot);
  await fs.mkdir(path.join(driveRoot, ".cat"), { recursive: true });
  await fs.writeFile(p, JSON.stringify(state, null, 2), "utf-8");
}