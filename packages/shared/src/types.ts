export interface RepoEntry {
  name: string;
  remoteUrl: string;
  localPath: string;
  readOnly?: boolean;
  lastSyncedAt?: string;   // ISO timestamp
  lastSyncedSha?: string;  // HEAD commit sha at last successful sync
  sizeBytes?: number;      // for drive-capacity UI
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

export const STATE_FILENAME = "state.json";