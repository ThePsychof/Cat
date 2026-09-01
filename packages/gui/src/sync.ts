import { filesystem } from "@neutralinojs/lib";
import { cloneRepo, pull, getHeadSha, type ProgressCallback } from "./git.js";
import { readState, writeState, type RepoEntry } from "./state.js";

async function pathExists(p: string): Promise<boolean> {
  try {
    await filesystem.getStats(p);
    return true;
  } catch {
    return false;
  }
}

async function dirSizeBytes(driveRoot: string, repoDir: string): Promise<number | undefined> {
  // Neutralino's filesystem API has no recursive size helper; skipping for now
  // rather than hand-rolling a recursive walk here. Revisit if drive-capacity
  // UI needs it — os.execCommand("du -sb <dir>") is the pragmatic fallback
  // on macOS/Linux; Windows needs a different approach.
  return undefined;
}

export interface SyncResult {
  repo: RepoEntry;
  action: "cloned" | "pulled";
}

export async function syncRepo(
  driveRoot: string,
  repo: RepoEntry,
  authorName: string,
  authorEmail: string,
  token: string | undefined,
  onProgress?: ProgressCallback
): Promise<SyncResult> {
  const fullPath = `${driveRoot}/${repo.localPath}`;
  const exists = await pathExists(`${fullPath}/.git`);

  let action: "cloned" | "pulled";
  if (!exists) {
    await cloneRepo(driveRoot, repo.remoteUrl, repo.localPath, token, onProgress);
    action = "cloned";
  } else {
    await pull(driveRoot, repo.localPath, authorName, authorEmail, token, onProgress);
    action = "pulled";
  }

  const sha = await getHeadSha(driveRoot, repo.localPath);
  const updatedRepo: RepoEntry = {
    ...repo,
    lastSyncedAt: new Date().toISOString(),
    lastSyncedSha: sha,
    sizeBytes: await dirSizeBytes(driveRoot, repo.localPath),
  };

  return { repo: updatedRepo, action };
}

/**
 * Syncs every repo in state.json sequentially, persisting progress after
 * each repo so a crash or drive-yank partway through doesn't lose earlier
 * successes. Returns per-repo outcomes including failures.
 */
export interface SyncAllResult {
  name: string;
  status: "cloned" | "pulled" | "failed";
  error?: string;
}

export async function syncAll(
  driveRoot: string,
  authorName: string,
  authorEmail: string,
  token: string | undefined,
  onRepoProgress?: (repoName: string, event: { phase: string; loaded: number; total?: number }) => void
): Promise<SyncAllResult[]> {
  const state = await readState(driveRoot);
  const results: SyncAllResult[] = [];

  for (const repo of state.repos) {
    try {
      const { repo: updated, action } = await syncRepo(
        driveRoot,
        repo,
        authorName,
        authorEmail,
        token,
        onRepoProgress ? (e) => onRepoProgress(repo.name, e) : undefined
      );

      // Persist immediately after each repo, not batched at the end.
      const idx = state.repos.findIndex((r) => r.name === repo.name);
      state.repos[idx] = updated;
      await writeState(driveRoot, state);

      results.push({ name: repo.name, status: action });
    } catch (err) {
      results.push({
        name: repo.name,
        status: "failed",
        error: err instanceof Error ? err.message : String(err),
      });
      // Continue to the next repo rather than aborting the whole backup run.
    }
  }

  return results;
}