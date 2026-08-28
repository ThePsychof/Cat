import { promises as fs } from "node:fs";
import path from "node:path";

export interface RepoEntry {
  name: string;
  remoteUrl: string;
  localPath: string; // relative to drive root
}

export interface GitProfile {
  name: string;       // profile label, e.g. "personal" or "work"
  userName: string;   // git user.name
  userEmail: string;  // git user.email
}

export interface CatState {
  version: string;
  selectedOS: string[];
  activeProfile: string | null;
  profiles: GitProfile[];
  repos: RepoEntry[];
}

const STATE_FILENAME = "state.json";

function statePath(driveRoot: string): string {
  return path.join(driveRoot, ".cat", STATE_FILENAME);
}

export async function readState(driveRoot: string): Promise<CatState> {
  const p = statePath(driveRoot);
  try {
    const raw = await fs.readFile(p, "utf-8");
    return JSON.parse(raw) as CatState;
  } catch {
    // No state yet — return a fresh default rather than throwing.
    return { version: "0.0.1", selectedOS: [], activeProfile: null, profiles: [], repos: [] };
  }
}

export async function writeState(driveRoot: string, state: CatState): Promise<void> {
  const p = statePath(driveRoot);
  await fs.mkdir(path.dirname(p), { recursive: true });
  await fs.writeFile(p, JSON.stringify(state, null, 2), "utf-8");
}