import { filesystem } from "@neutralinojs/lib";
import { joinPath, ensureDir } from "./neutralino-paths.js";

export interface RepoEntry {
  name: string;
  remoteUrl: string;
  localPath: string;
  readOnly?: boolean;
}

export interface GitProfile {
  name: string;
  userName: string;
  userEmail: string;
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
  return joinPath(driveRoot, ".cat", STATE_FILENAME);
}

export async function readState(driveRoot: string): Promise<CatState> {
  const p = statePath(driveRoot);
  try {
    const raw = await filesystem.readFile(p);
    return JSON.parse(raw) as CatState;
  } catch {
    return { version: "0.0.1", selectedOS: [], activeProfile: null, profiles: [], repos: [] };
  }
}

export async function writeState(driveRoot: string, state: CatState): Promise<void> {
  const p = statePath(driveRoot);
  await ensureDir(joinPath(driveRoot, ".cat"));
  await filesystem.writeFile(p, JSON.stringify(state, null, 2));
}