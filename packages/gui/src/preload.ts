import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("cat", {
  version: () => ipcRenderer.invoke("cat:getVersion"),
  getState: () => ipcRenderer.invoke("cat:getState"),
  saveState: (state: unknown) => ipcRenderer.invoke("cat:saveState", state),
  cloneRepo: (remoteUrl: string, targetDir: string, profileName?: string, passphrase?: string) =>
    ipcRenderer.invoke("cat:cloneRepo", remoteUrl, targetDir, profileName, passphrase),
  pull: (
    repoDir: string,
    authorName: string,
    authorEmail: string,
    profileName?: string,
    passphrase?: string
  ) => ipcRenderer.invoke("cat:pull", repoDir, authorName, authorEmail, profileName, passphrase),
  push: (repoDir: string, profileName?: string, passphrase?: string) =>
    ipcRenderer.invoke("cat:push", repoDir, profileName, passphrase),
  setRepoIdentity: (repoDir: string, userName: string, userEmail: string) =>
    ipcRenderer.invoke("cat:setRepoIdentity", repoDir, userName, userEmail),
  getChangedFiles: (repoDir: string) => ipcRenderer.invoke("cat:getChangedFiles", repoDir),
  stageAll: (repoDir: string) => ipcRenderer.invoke("cat:stageAll", repoDir),
  listBranches: (repoDir: string) => ipcRenderer.invoke("cat:listBranches", repoDir),
  getCurrentBranch: (repoDir: string) => ipcRenderer.invoke("cat:getCurrentBranch", repoDir),
  checkoutBranch: (repoDir: string, branchName: string) =>
    ipcRenderer.invoke("cat:checkoutBranch", repoDir, branchName),
  createBranch: (repoDir: string, branchName: string) =>
    ipcRenderer.invoke("cat:createBranch", repoDir, branchName),
  commit: (repoDir: string, message: string, authorName: string, authorEmail: string) =>
    ipcRenderer.invoke("cat:commit", repoDir, message, authorName, authorEmail),
  setToken: (profileName: string, token: string, passphrase: string) =>
    ipcRenderer.invoke("cat:setToken", profileName, token, passphrase),
  checkForUpdate: () => ipcRenderer.invoke("cat:checkForUpdate"),
  downloadUpdate: (manifest: unknown) => ipcRenderer.invoke("cat:downloadUpdate", manifest),
});