import fs from "node:fs";
import path from "node:path";
import git from "isomorphic-git";
import http from "isomorphic-git/http/node";
import { readState } from "./state.js";

async function assertWritable(driveRoot: string, repoDir: string): Promise<void> {
  const state = await readState(driveRoot);
  const entry = state.repos.find((r) => r.localPath === repoDir);
  if (entry?.readOnly) {
    throw new Error(
      `"${entry.name}" is set to read-only in Cat — commits and pushes are disabled for this repo.`
    );
  }
}

function onAuth(token?: string) {
  if (!token) return undefined;
  return () => ({ username: token, password: "x-oauth-basic" });
}

export async function cloneRepo(
  driveRoot: string,
  remoteUrl: string,
  targetDir: string,
  token?: string
): Promise<void> {
  const dir = path.join(driveRoot, targetDir);
  await git.clone({
    fs,
    http,
    dir,
    url: remoteUrl,
    onAuth: onAuth(token),
  });
}

export async function pull(
  driveRoot: string,
  repoDir: string,
  authorName: string,
  authorEmail: string,
  token?: string
): Promise<string> {
  const dir = path.join(driveRoot, repoDir);
  await git.pull({
    fs,
    http,
    dir,
    author: { name: authorName, email: authorEmail },
    onAuth: onAuth(token),
  });
  return "Pull complete.";
}

export async function push(
  driveRoot: string,
  repoDir: string,
  token?: string
): Promise<string> {
  await assertWritable(driveRoot, repoDir);
  const dir = path.join(driveRoot, repoDir);
  const result = await git.push({
    fs,
    http,
    dir,
    onAuth: onAuth(token),
  });
  return JSON.stringify(result, null, 2);
}

export async function setLocalIdentity(
  driveRoot: string,
  repoDir: string,
  userName: string,
  userEmail: string
): Promise<void> {
  const dir = path.join(driveRoot, repoDir);
  await git.setConfig({ fs, dir, path: "user.name", value: userName });
  await git.setConfig({ fs, dir, path: "user.email", value: userEmail });
}

export interface FileChange {
  filepath: string;
  status: "unmodified" | "modified" | "added" | "deleted" | "unknown";
}

// isomorphic-git's statusMatrix returns [filepath, headStatus, workdirStatus, stageStatus]
// for every tracked/untracked file. We collapse that into a simple label per file.
function interpretMatrixRow(row: [string, number, number, number]): FileChange {
  const [filepath, head, workdir, stage] = row;

  if (head === 0 && workdir === 2) {
    return { filepath, status: "added" };
  }
  if (head === 1 && workdir === 0) {
    return { filepath, status: "deleted" };
  }
  if (head === 1 && workdir === 2 && stage === 1) {
    return { filepath, status: "modified" };
  }
  if (head === 1 && workdir === 1 && stage === 1) {
    return { filepath, status: "unmodified" };
  }
  return { filepath, status: "unknown" };
}

export async function getChangedFiles(
  driveRoot: string,
  repoDir: string
): Promise<FileChange[]> {
  const dir = path.join(driveRoot, repoDir);
  const matrix = await git.statusMatrix({ fs, dir });
  return matrix
    .map((row: [string, number, number, number]) => interpretMatrixRow(row))
    .filter((change: FileChange) => change.status !== "unmodified");
}

export async function stageFile(
  driveRoot: string,
  repoDir: string,
  filepath: string
): Promise<void> {
  const dir = path.join(driveRoot, repoDir);
  await git.add({ fs, dir, filepath });
}

export async function stageAll(driveRoot: string, repoDir: string): Promise<void> {
  const changes = await getChangedFiles(driveRoot, repoDir);
  const dir = path.join(driveRoot, repoDir);
  for (const change of changes) {
    if (change.status === "deleted") {
      await git.remove({ fs, dir, filepath: change.filepath });
    } else {
      await git.add({ fs, dir, filepath: change.filepath });
    }
  }
}

export async function commit(
  driveRoot: string,
  repoDir: string,
  message: string,
  authorName: string,
  authorEmail: string
): Promise<string> {
  await assertWritable(driveRoot, repoDir);
  const dir = path.join(driveRoot, repoDir);
  const sha = await git.commit({
    fs,
    dir,
    message,
    author: { name: authorName, email: authorEmail },
  });
  return sha;
}

export async function listBranches(driveRoot: string, repoDir: string): Promise<string[]> {
  const dir = path.join(driveRoot, repoDir);
  return git.listBranches({ fs, dir });
}

export async function getCurrentBranch(
  driveRoot: string,
  repoDir: string
): Promise<string | undefined> {
  const dir = path.join(driveRoot, repoDir);
  const branch = await git.currentBranch({ fs, dir, fullname: false });
  return branch ?? undefined;
}

export async function checkoutBranch(
  driveRoot: string,
  repoDir: string,
  branchName: string
): Promise<void> {
  const dir = path.join(driveRoot, repoDir);
  await git.checkout({ fs, dir, ref: branchName });
}

export async function createBranch(
  driveRoot: string,
  repoDir: string,
  branchName: string
): Promise<void> {
  const dir = path.join(driveRoot, repoDir);
  await git.branch({ fs, dir, ref: branchName, checkout: true });
}