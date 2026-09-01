import git from "isomorphic-git";
import http from "isomorphic-git/http/web";
import { neutralinoFs } from "./neutralino-fs.js";

const fs = neutralinoFs;

function onAuth(token?: string) {
  if (!token) return undefined;
  return () => ({ username: token, password: "x-oauth-basic" });
}

export interface ProgressEvent {
  phase: string;
  loaded: number;
  total?: number;
}

export type ProgressCallback = (event: ProgressEvent) => void;

export async function cloneRepo(
  driveRoot: string,
  remoteUrl: string,
  targetDir: string,
  token?: string,
  onProgress?: ProgressCallback
): Promise<void> {
  const dir = `${driveRoot}/${targetDir}`;
  await git.clone({
    fs,
    http,
    dir,
    url: remoteUrl,
    corsProxy: undefined,
    singleBranch: false, // backup tool: capture all branches, not just the default
    onAuth: onAuth(token),
    onProgress: onProgress
      ? (e) => onProgress({ phase: e.phase, loaded: e.loaded, total: e.total })
      : undefined,
  });
}

export async function pull(
  driveRoot: string,
  repoDir: string,
  authorName: string,
  authorEmail: string,
  token?: string,
  onProgress?: ProgressCallback
): Promise<string> {
  const dir = `${driveRoot}/${repoDir}`;
  await git.pull({
    fs,
    http,
    dir,
    author: { name: authorName, email: authorEmail },
    onAuth: onAuth(token),
    onProgress: onProgress
      ? (e) => onProgress({ phase: e.phase, loaded: e.loaded, total: e.total })
      : undefined,
  });
  return "Pull complete.";
}

export async function push(
  driveRoot: string,
  repoDir: string,
  token?: string
): Promise<string> {
  const dir = `${driveRoot}/${repoDir}`;
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
  const dir = `${driveRoot}/${repoDir}`;
  await git.setConfig({ fs, dir, path: "user.name", value: userName });
  await git.setConfig({ fs, dir, path: "user.email", value: userEmail });
}

export interface FileChange {
  filepath: string;
  status: "unmodified" | "modified" | "added" | "deleted" | "unknown";
}

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
  const dir = `${driveRoot}/${repoDir}`;
  const matrix = await git.statusMatrix({ fs, dir });
  return matrix
    .map((row: [string, number, number, number]) => interpretMatrixRow(row))
    .filter((change: FileChange) => change.status !== "unmodified");
}

export async function listFiles(driveRoot: string, repoDir: string): Promise<string[]> {
  const dir = `${driveRoot}/${repoDir}`;
  const files = await git.listFiles({ fs, dir });
  return files;
}

export async function getCommitLog(
  driveRoot: string,
  repoDir: string,
  maxCount = 20
): Promise<Array<{ sha: string; author: string; email: string; date: string; message: string }>> {
  const dir = `${driveRoot}/${repoDir}`;
  const entries = await git.log({ fs, dir, depth: maxCount });
  return entries.map((entry) => {
    const ts = entry.commit.author.timestamp;
    const date = new Date(ts * 1000).toISOString();
    return {
      sha: entry.oid,
      author: entry.commit.author.name,
      email: entry.commit.author.email,
      date,
      message: entry.commit.message,
    };
  });
}

export async function stageAll(driveRoot: string, repoDir: string): Promise<void> {
  const changes = await getChangedFiles(driveRoot, repoDir);
  const dir = `${driveRoot}/${repoDir}`;
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
  const dir = `${driveRoot}/${repoDir}`;
  const sha = await git.commit({
    fs,
    dir,
    message,
    author: { name: authorName, email: authorEmail },
  });
  return sha;
}

export async function listBranches(driveRoot: string, repoDir: string): Promise<string[]> {
  const dir = `${driveRoot}/${repoDir}`;
  return git.listBranches({ fs, dir });
}

export async function getCurrentBranch(
  driveRoot: string,
  repoDir: string
): Promise<string | undefined> {
  const dir = `${driveRoot}/${repoDir}`;
  const branch = await git.currentBranch({ fs, dir, fullname: false });
  return branch ?? undefined;
}

export async function getHeadSha(driveRoot: string, repoDir: string): Promise<string | undefined> {
  const dir = `${driveRoot}/${repoDir}`;
  try {
    return await git.resolveRef({ fs, dir, ref: "HEAD" });
  } catch {
    return undefined;
  }
}

export async function checkoutBranch(
  driveRoot: string,
  repoDir: string,
  branchName: string
): Promise<void> {
  const dir = `${driveRoot}/${repoDir}`;
  await git.checkout({ fs, dir, ref: branchName });
}

export async function createBranch(
  driveRoot: string,
  repoDir: string,
  branchName: string
): Promise<void> {
  const dir = `${driveRoot}/${repoDir}`;
  await git.branch({ fs, dir, ref: branchName, checkout: true });
}