import { app, BrowserWindow, ipcMain } from "electron";
import path from "node:path";
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

const CURRENT_VERSION = "0.0.1";

function createWindow(): void {
  const driveLabel = path.parse(path.resolve(DRIVE_ROOT)).root.replace(/[\\/]$/, "") || DRIVE_ROOT;

  const win = new BrowserWindow({
    width: 1000,
    height: 700,
    title: `Cat — ${driveLabel}`,
    icon: path.join(__dirname, "..", "build", "icon.png"), // taskbar/alt-tab icon
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  win.loadFile(path.join(__dirname, "..", "renderer", "index.html"));
}

function resolveDriveRoot(): string {
  if (app.isPackaged) {
    // electron-builder's Windows "portable" target self-extracts the app
    // into a temp folder and runs it from there — app.getPath("exe") then
    // points into TEMP, not onto the flash drive. That's why clones/pulls
    // "succeeded" but never showed up on the drive: they were writing to a
    // temp folder that gets wiped when the app closes.
    //
    // electron-builder sets PORTABLE_EXECUTABLE_DIR to the real folder the
    // portable .exe was launched from — use that instead.
    if (process.env.PORTABLE_EXECUTABLE_DIR) {
      return process.env.PORTABLE_EXECUTABLE_DIR;
    }
    // Linux AppImage similarly mounts/runs from a temp mountpoint; APPIMAGE
    // holds the real path to the .AppImage file itself.
    if (process.env.APPIMAGE) {
      return path.dirname(process.env.APPIMAGE);
    }
    // macOS .app bundles (and anything non-self-extracting) run in place,
    // so this fallback is correct there.
    return path.dirname(app.getPath("exe"));
  }
  return process.env.CAT_DRIVE_ROOT ?? path.resolve(process.cwd(), "dev-drive");
}

const DRIVE_ROOT = resolveDriveRoot();

ipcMain.handle("cat:getState", async () => {
  return readState(DRIVE_ROOT);
});

ipcMain.handle("cat:saveState", async (_event, state: CatState) => {
  await writeState(DRIVE_ROOT, state);
});

ipcMain.handle(
  "cat:cloneRepo",
  async (
    _event,
    remoteUrl: string,
    targetDir: string,
    profileName?: string,
    passphrase?: string,
    readOnly = false
  ) => {
    const token =
      profileName && passphrase ? await getToken(DRIVE_ROOT, profileName, passphrase) : null;
    await cloneRepo(DRIVE_ROOT, remoteUrl, targetDir, token ?? undefined);

    // Persist read-only status right here, at the source of truth, rather
    // than relying on the renderer to separately call cat:saveState with
    // the right shape — this way assertWritable() in git.ts always sees it.
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
  }
);

ipcMain.handle(
  "cat:setRepoReadOnly",
  async (_event, repoDir: string, readOnly: boolean) => {
    const state = await readState(DRIVE_ROOT);
    const entry = state.repos.find((r) => r.localPath === repoDir);
    if (entry) {
      entry.readOnly = readOnly;
      await writeState(DRIVE_ROOT, state);
    }
  }
);

ipcMain.handle(
  "cat:pull",
  async (
    _event,
    repoDir: string,
    authorName: string,
    authorEmail: string,
    profileName?: string,
    passphrase?: string
  ) => {
    const token =
      profileName && passphrase ? await getToken(DRIVE_ROOT, profileName, passphrase) : null;
    return pull(DRIVE_ROOT, repoDir, authorName, authorEmail, token ?? undefined);
  }
);

ipcMain.handle(
  "cat:push",
  async (_event, repoDir: string, profileName?: string, passphrase?: string) => {
    const token =
      profileName && passphrase ? await getToken(DRIVE_ROOT, profileName, passphrase) : null;
    return push(DRIVE_ROOT, repoDir, token ?? undefined);
  }
);

ipcMain.handle(
  "cat:setRepoIdentity",
  async (_event, repoDir: string, userName: string, userEmail: string) => {
    await setLocalIdentity(DRIVE_ROOT, repoDir, userName, userEmail);
  }
);

ipcMain.handle("cat:getChangedFiles", async (_event, repoDir: string) => {
  return getChangedFiles(DRIVE_ROOT, repoDir);
});

ipcMain.handle("cat:stageAll", async (_event, repoDir: string) => {
  await stageAll(DRIVE_ROOT, repoDir);
});

ipcMain.handle("cat:listBranches", async (_event, repoDir: string) => {
  return listBranches(DRIVE_ROOT, repoDir);
});

ipcMain.handle("cat:getCurrentBranch", async (_event, repoDir: string) => {
  return getCurrentBranch(DRIVE_ROOT, repoDir);
});

ipcMain.handle("cat:checkoutBranch", async (_event, repoDir: string, branchName: string) => {
  await checkoutBranch(DRIVE_ROOT, repoDir, branchName);
});

ipcMain.handle("cat:createBranch", async (_event, repoDir: string, branchName: string) => {
  await createBranch(DRIVE_ROOT, repoDir, branchName);
});

ipcMain.handle(
  "cat:commit",
  async (
    _event,
    repoDir: string,
    message: string,
    authorName: string,
    authorEmail: string
  ) => {
    return commit(DRIVE_ROOT, repoDir, message, authorName, authorEmail);
  }
);

ipcMain.handle(
  "cat:setToken",
  async (_event, profileName: string, token: string, passphrase: string) => {
    await setToken(DRIVE_ROOT, profileName, token, passphrase);
  }
);

app.whenReady().then(async () => {
  // Apply any update staged from a previous session before the UI loads.
  try {
    await applyPendingUpdate(DRIVE_ROOT);
  } catch (err) {
    console.error("Failed to apply pending update:", err);
  }

  createWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

ipcMain.handle("cat:checkForUpdate", async () => {
  return checkForUpdate(CURRENT_VERSION);
});

ipcMain.handle("cat:downloadUpdate", async (_event, manifest) => {
  return downloadUpdate(DRIVE_ROOT, manifest);
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});