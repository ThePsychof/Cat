// isomorphic-git expects a Node-fs-shaped object. This adapts Neutralino's
// real filesystem API (verified against @neutralinojs/lib's shipped type
// definitions) to that shape, with two correctness-critical translations:
//
// 1. Binary safety: git's internal objects (loose objects, packfiles, the
//    index) are binary data. Neutralino's readFile/writeFile only handle
//    strings — using them on binary git data would corrupt it via encoding
//    round-trips. Every read/write here goes through the *Binary variants
//    (ArrayBuffer) instead.
//
// 2. Error code translation: isomorphic-git checks `err.code === 'ENOENT'`
//    (Node convention) in many places to mean "doesn't exist yet, proceed."
//    Neutralino throws its own codes (e.g. "NE_FS_NOPATHE") — left
//    untranslated, isomorphic-git would throw where it should silently
//    continue. translateError() below bridges this.

import { filesystem } from "@neutralinojs/lib";

interface NeutralinoErrorLike {
  code?: string;
  message?: string;
}

function translateError(err: unknown, path: string): NodeJS.ErrnoException {
  const neuErr = err as NeutralinoErrorLike;
  const nodeStyle = new Error(neuErr?.message ?? String(err)) as NodeJS.ErrnoException;

  // NE_FS_NOPATHE covers "path doesn't exist" across read/stat/readdir calls
  // in Neutralino's filesystem API — the one case isomorphic-git specifically
  // checks for via err.code === 'ENOENT'.
  if (neuErr?.code === "NE_FS_NOPATHE") {
    nodeStyle.code = "ENOENT";
  } else {
    nodeStyle.code = neuErr?.code ?? "EUNKNOWN";
  }

  nodeStyle.path = path;
  return nodeStyle;
}

function toUint8Array(buffer: ArrayBuffer): Uint8Array {
  return new Uint8Array(buffer);
}

function toArrayBuffer(data: Uint8Array | string): ArrayBuffer {
  if (typeof data === "string") {
    return new TextEncoder().encode(data).buffer as ArrayBuffer;
  }
  // Copy into a fresh, exactly-sized ArrayBuffer — data.buffer may be a
  // larger, shared backing buffer (common with Buffer/Uint8Array views).
  return data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength) as ArrayBuffer;
}

async function readFile(
  path: string,
  options?: { encoding?: string }
): Promise<Uint8Array | string> {
  try {
    const buffer = await filesystem.readBinaryFile(path);
    const bytes = toUint8Array(buffer);
    if (options?.encoding === "utf8") {
      return new TextDecoder().decode(bytes);
    }
    return bytes;
  } catch (err) {
    throw translateError(err, path);
  }
}

async function writeFile(path: string, data: Uint8Array | string): Promise<void> {
  try {
    await filesystem.writeBinaryFile(path, toArrayBuffer(data));
  } catch (err) {
    throw translateError(err, path);
  }
}

async function unlink(path: string): Promise<void> {
  try {
    await filesystem.remove(path);
  } catch (err) {
    throw translateError(err, path);
  }
}

async function readdir(path: string): Promise<string[]> {
  try {
    const entries = await filesystem.readDirectory(path);
    // Neutralino's DirectoryEntry.entry is the bare filename — isomorphic-git
    // expects readdir() to return just names, matching Node's fs.readdir.
    return entries.map((e) => e.entry).filter((name) => name !== "." && name !== "..");
  } catch (err) {
    throw translateError(err, path);
  }
}

async function mkdir(path: string): Promise<void> {
  try {
    await filesystem.createDirectory(path);
  } catch (err) {
    // isomorphic-git creates the same directories repeatedly across many
    // operations and expects EEXIST to be silently tolerated. Neutralino's
    // ErrorCode enum has no distinct "already exists" code, so we check
    // directly rather than guessing at the error shape.
    const stillMissing = await stat(path).catch(() => null);
    if (stillMissing?.type === "dir") {
      return;
    }
    throw translateError(err, path);
  }
}

async function rmdir(path: string): Promise<void> {
  try {
    await filesystem.remove(path);
  } catch (err) {
    // isomorphic-git calls rmdir opportunistically during cleanup, often on
    // directories that may already be gone or non-empty — treated as
    // best-effort, matching common fs-adapter conventions for this call.
    const err2 = translateError(err, path);
    if (err2.code !== "ENOENT") {
      throw err2;
    }
  }
}

interface GitStat {
  type: "file" | "dir" | "symlink";
  mode: number;
  size: number;
  ino: number;
  mtimeMs: number;
  ctimeMs: number;
  uid: number;
  gid: number;
  dev: number;
}

function toGitStat(s: { size: number; isFile: boolean; isDirectory: boolean; modifiedAt: number }): GitStat {
  return {
    type: s.isDirectory ? "dir" : "file",
    mode: s.isDirectory ? 0o040000 : 0o100644,
    size: s.size,
    ino: 0,
    mtimeMs: s.modifiedAt,
    ctimeMs: s.modifiedAt,
    uid: 1,
    gid: 1,
    dev: 1,
  };
}

async function stat(path: string): Promise<GitStat> {
  try {
    const s = await filesystem.getStats(path);
    return toGitStat(s);
  } catch (err) {
    throw translateError(err, path);
  }
}

// Neutralino's filesystem API has no symlink primitives — lstat falls back
// to stat, meaning symlinks in a cloned repo's tree won't be detected as
// such. Known v1 limitation: repos containing symlinks won't fully
// round-trip. Everything else (the overwhelming majority of repos) is fine.
const lstat = stat;

export const neutralinoFs = {
  promises: {
    readFile,
    writeFile,
    unlink,
    readdir,
    mkdir,
    rmdir,
    stat,
    lstat,
  },
};