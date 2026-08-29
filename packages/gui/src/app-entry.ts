import { init, events, app as neuApp } from "@neutralinojs/lib";
import { readState, writeState, CatState } from "./state.js";
import {
  cloneRepo,
  pull,
  push,
  setLocalIdentity,
  getChangedFiles,
  stageAll,
  commit,
  listBranches,
  getCurrentBranch,
  checkoutBranch,
  createBranch,
} from "./git.js";
import { getToken, setToken } from "./credentials.js";
import { checkForUpdate, downloadUpdate, applyPendingUpdate } from "./update.js";

// NL_PATH is the real folder the running binary lives in. Unlike Electron's
// portable-.exe format (which self-extracted to a TEMP folder, requiring the
// PORTABLE_EXECUTABLE_DIR workaround), Neutralino's binary runs directly from
// wherever it's placed — so NL_PATH is already the correct drive root with no
// special-casing needed.
declare const NL_PATH: string;
const DRIVE_ROOT = NL_PATH;

async function main() {
  await init();

  events.on("windowClose", () => neuApp.exit());

  try {
    await applyPendingUpdate(DRIVE_ROOT);
  } catch (err) {
    console.error("Failed to apply pending update:", err);
  }

  (window as any).cat = {
    version: async () => (window as any).NL_APPVERSION as string,
    getState: () => readState(DRIVE_ROOT),
    saveState: (state: CatState) => writeState(DRIVE_ROOT, state),

    cloneRepo: async (
      remoteUrl: string,
      targetDir: string,
      profileName?: string,
      passphrase?: string,
      readOnly = false
    ) => {
      const token =
        profileName && passphrase ? await getToken(DRIVE_ROOT, profileName, passphrase) : null;
      await cloneRepo(DRIVE_ROOT, remoteUrl, targetDir, token ?? undefined);

      const state = await readState(DRIVE_ROOT);
      const name = targetDir.split(/[\\/]/).pop() ?? targetDir;
      const existing = state.repos.find((r) => r.localPath === targetDir);
      if (existing) {
        existing.readOnly = readOnly;
        existing.remoteUrl = remoteUrl;
      } else {
        state.repos.push({ name, remoteUrl, localPath: targetDir, readOnly });
      }
      await writeState(DRIVE_ROOT, state);
    },

    pull: async (
      repoDir: string,
      authorName: string,
      authorEmail: string,
      profileName?: string,
      passphrase?: string
    ) => {
      const token =
        profileName && passphrase ? await getToken(DRIVE_ROOT, profileName, passphrase) : null;
      return pull(DRIVE_ROOT, repoDir, authorName, authorEmail, token ?? undefined);
    },

    push: async (repoDir: string, profileName?: string, passphrase?: string) => {
      const token =
        profileName && passphrase ? await getToken(DRIVE_ROOT, profileName, passphrase) : null;
      return push(DRIVE_ROOT, repoDir, token ?? undefined);
    },

    setRepoIdentity: (repoDir: string, userName: string, userEmail: string) =>
      setLocalIdentity(DRIVE_ROOT, repoDir, userName, userEmail),

    getChangedFiles: (repoDir: string) => getChangedFiles(DRIVE_ROOT, repoDir),
    stageAll: (repoDir: string) => stageAll(DRIVE_ROOT, repoDir),
    listBranches: (repoDir: string) => listBranches(DRIVE_ROOT, repoDir),
    getCurrentBranch: (repoDir: string) => getCurrentBranch(DRIVE_ROOT, repoDir),
    checkoutBranch: (repoDir: string, branchName: string) =>
      checkoutBranch(DRIVE_ROOT, repoDir, branchName),
    createBranch: (repoDir: string, branchName: string) =>
      createBranch(DRIVE_ROOT, repoDir, branchName),
    commit: (repoDir: string, message: string, authorName: string, authorEmail: string) =>
      commit(DRIVE_ROOT, repoDir, message, authorName, authorEmail),
    setToken: (profileName: string, token: string, passphrase: string) =>
      setToken(DRIVE_ROOT, profileName, token, passphrase),

    checkForUpdate: () => checkForUpdate((window as any).NL_APPVERSION),
    downloadUpdate: (manifest: unknown) => downloadUpdate(DRIVE_ROOT, manifest as any),
  };

  document.dispatchEvent(new Event("cat-ready"));
}

main();