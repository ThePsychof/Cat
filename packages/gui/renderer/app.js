const repoListEl = document.getElementById("repo-list");
const statusEl = document.getElementById("status");
const cloneUrlInput = document.getElementById("clone-url");
const cloneBtn = document.getElementById("clone-btn");
const profileSelectEl = document.getElementById("profile-select");
const newProfileBtn = document.getElementById("new-profile-btn");

const modalOverlayEl = document.getElementById("modal-overlay");
const modalTitleEl = document.getElementById("modal-title");
const modalFieldsEl = document.getElementById("modal-fields");
const modalOkBtn = document.getElementById("modal-ok-btn");
const modalCancelBtn = document.getElementById("modal-cancel-btn");

// Replacement for window.prompt(), which Electron does not support.
// Pass an array of field labels; resolves to an array of entered values,
// or null if cancelled. A single-label call behaves like a single prompt.
function showPromptModal(labels) {
  return new Promise((resolve) => {
    modalTitleEl.textContent = Array.isArray(labels) ? "Enter details" : labels;
    modalFieldsEl.innerHTML = "";

    const fieldLabels = Array.isArray(labels) ? labels : [labels];
    const inputs = fieldLabels.map((label) => {
      const input = document.createElement("input");
      input.type = label.toLowerCase().includes("token") || label.toLowerCase().includes("passphrase")
        ? "password"
        : "text";
      input.placeholder = label;
      modalFieldsEl.appendChild(input);
      return input;
    });

    modalOverlayEl.style.display = "flex";
    inputs[0].focus();

    function cleanup(result) {
      modalOverlayEl.style.display = "none";
      modalOkBtn.onclick = null;
      modalCancelBtn.onclick = null;
      resolve(result);
    }

    modalOkBtn.onclick = () => {
      const values = inputs.map((i) => i.value.trim());
      if (values.some((v) => !v)) {
        cleanup(null);
        return;
      }
      cleanup(values.length === 1 ? values[0] : values);
    };

    modalCancelBtn.onclick = () => cleanup(null);
  });
}

// Replacement for window.confirm(), same reason.
function showConfirmModal(message) {
  return new Promise((resolve) => {
    modalTitleEl.textContent = message;
    modalFieldsEl.innerHTML = "";
    modalOverlayEl.style.display = "flex";

    function cleanup(result) {
      modalOverlayEl.style.display = "none";
      modalOkBtn.onclick = null;
      modalCancelBtn.onclick = null;
      resolve(result);
    }

    modalOkBtn.onclick = () => cleanup(true);
    modalCancelBtn.onclick = () => cleanup(false);
  });
}

let currentState = null;
let sessionPassphrase = null;
let hadRepos = false;

async function ensurePassphrase() {
  if (!sessionPassphrase) {
    sessionPassphrase = await showPromptModal("Enter your Cat passphrase (unlocks stored credentials):");
  }
  return sessionPassphrase;
}

function resetPassphrase() {
  sessionPassphrase = null;
}

function setStatus(text, isError = false) {
  statusEl.textContent = text;
  statusEl.classList.toggle("error", isError);
}

function repoDirName(remoteUrl) {
  const parts = remoteUrl.replace(/\/$/, "").split("/");
  return parts[parts.length - 1].replace(/\.git$/, "");
}

async function loadState() {
  currentState = await window.cat.getState();
  renderProfileSelect(currentState);
  renderRepoList(currentState);
  return currentState;
}

function renderProfileSelect(state) {
  profileSelectEl.innerHTML = "";

  for (const profile of state.profiles) {
    const opt = document.createElement("option");
    opt.value = profile.name;
    opt.textContent = profile.name;
    if (profile.name === state.activeProfile) opt.selected = true;
    profileSelectEl.appendChild(opt);
  }
}

function getActiveProfile(state) {
  return state.profiles.find((p) => p.name === state.activeProfile) ?? null;
}

async function handleProfileChange() {
  currentState.activeProfile = profileSelectEl.value;
  await window.cat.saveState(currentState);
  setStatus(`Switched active profile to ${currentState.activeProfile}.`);
}

async function handleNewProfile() {
  const values = await showPromptModal([
    "Profile name (e.g. personal, work)",
    "Git user.name",
    "Git user.email",
    "GitHub Personal Access Token",
  ]);
  if (!values) return;
  const [name, userName, userEmail, token] = values;

  const passphrase = await ensurePassphrase();
  if (!passphrase) {
    setStatus("Profile creation cancelled — passphrase required to store credentials.");
    return;
  }

  currentState.profiles.push({ name, userName, userEmail });
  currentState.activeProfile = name;
  await window.cat.saveState(currentState);

  try {
    await window.cat.setToken(name, token, passphrase);
  } catch (err) {
    setStatus(`Failed to store credentials: ${err.message}`);
    return;
  }

  renderProfileSelect(currentState);
  setStatus(`Created and switched to profile ${name}.`);
}

profileSelectEl.addEventListener("change", handleProfileChange);
newProfileBtn.addEventListener("click", handleNewProfile);

function renderRepoList(state) {
  repoListEl.innerHTML = "";

  const emptyStateEl = document.getElementById("empty-state");
  const isWaking = state.repos.length > 0 && !hadRepos;

  if (isWaking) {
    emptyStateEl.classList.add("waking");
    repoListEl.classList.add("entering");
    setTimeout(() => {
      emptyStateEl.style.display = "none";
      emptyStateEl.classList.remove("waking");
    }, 480);
  } else {
    emptyStateEl.style.display = state.repos.length === 0 ? "flex" : "none";
    repoListEl.style.display = state.repos.length === 0 ? "none" : "block";
  }

  hadRepos = state.repos.length > 0;

  for (const repo of state.repos) {
    const li = document.createElement("li");

    const label = document.createElement("span");
    label.className = "repo-name";
    label.textContent = repo.name;

    const actions = document.createElement("span");
    actions.className = "repo-actions";

    const changesBtn = document.createElement("button");
    changesBtn.textContent = "Changes";
    changesBtn.onclick = () => openChangesPanel(repo);

    const pullBtn = document.createElement("button");
    pullBtn.textContent = "Pull";
    pullBtn.onclick = () => handlePull(repo);

    const pushBtn = document.createElement("button");
    pushBtn.textContent = "Push";
    pushBtn.onclick = () => handlePush(repo);

    actions.appendChild(changesBtn);
    actions.appendChild(pullBtn);
    actions.appendChild(pushBtn);

    li.appendChild(label);
    li.appendChild(actions);
    repoListEl.appendChild(li);
  }
}

const changesPanelEl = document.getElementById("changes-panel");
const changesRepoNameEl = document.getElementById("changes-repo-name");
const changesFileListEl = document.getElementById("changes-file-list");
const commitMessageEl = document.getElementById("commit-message");
const commitBtn = document.getElementById("commit-btn");
const changesCloseBtn = document.getElementById("changes-close-btn");

let activeChangesRepo = null;

const branchSelectEl = document.getElementById("branch-select");
const newBranchBtn = document.getElementById("new-branch-btn");

async function openChangesPanel(repo) {
  activeChangesRepo = repo;
  changesRepoNameEl.textContent = `Changes: ${repo.name}`;
  changesPanelEl.style.display = "block";
  commitMessageEl.value = "";

  try {
    const changes = await window.cat.getChangedFiles(repo.localPath);
    renderChangesList(changes);
  } catch (err) {
    setStatus(`Failed to get changes for ${repo.name}: ${err.message}`);
  }

  try {
    await refreshBranches(repo);
  } catch (err) {
    setStatus(`Failed to load branches for ${repo.name}: ${err.message}`);
  }
}

async function refreshBranches(repo) {
  const branches = await window.cat.listBranches(repo.localPath);
  const current = await window.cat.getCurrentBranch(repo.localPath);

  branchSelectEl.innerHTML = "";
  for (const branch of branches) {
    const opt = document.createElement("option");
    opt.value = branch;
    opt.textContent = branch;
    if (branch === current) opt.selected = true;
    branchSelectEl.appendChild(opt);
  }
}

async function handleBranchChange() {
  if (!activeChangesRepo) return;
  const branchName = branchSelectEl.value;

  setStatus(`Switching to branch ${branchName}...`);
  try {
    await window.cat.checkoutBranch(activeChangesRepo.localPath, branchName);
    setStatus(`Switched to ${branchName}.`);

    const changes = await window.cat.getChangedFiles(activeChangesRepo.localPath);
    renderChangesList(changes);
  } catch (err) {
    setStatus(`Branch switch failed: ${err.message}`, true);
  }
}

async function handleNewBranch() {
  if (!activeChangesRepo) return;
  const branchName = await showPromptModal("New branch name:");
  if (!branchName) return;

  setStatus(`Creating branch ${branchName}...`);
  try {
    await window.cat.createBranch(activeChangesRepo.localPath, branchName);
    await refreshBranches(activeChangesRepo);
    setStatus(`Created and switched to ${branchName}.`);
  } catch (err) {
    setStatus(`Branch creation failed: ${err.message}`, true);
  }
}

branchSelectEl.addEventListener("change", handleBranchChange);
newBranchBtn.addEventListener("click", handleNewBranch);

function renderChangesList(changes) {
  changesFileListEl.innerHTML = "";

  if (changes.length === 0) {
    const li = document.createElement("li");
    li.textContent = "No changes.";
    changesFileListEl.appendChild(li);
    return;
  }

  for (const change of changes) {
    const li = document.createElement("li");

    const name = document.createElement("span");
    name.textContent = change.filepath;

    const tag = document.createElement("span");
    tag.className = "status-tag";
    tag.textContent = change.status;

    li.appendChild(name);
    li.appendChild(tag);
    changesFileListEl.appendChild(li);
  }
}

async function handleCommit() {
  if (!activeChangesRepo) return;

  const message = commitMessageEl.value.trim();
  if (!message) {
    setStatus("Commit message required.");
    return;
  }

  const profile = getActiveProfile(currentState);
  const authorName = profile ? profile.userName : "Cat User";
  const authorEmail = profile ? profile.userEmail : "cat@local";

  setStatus(`Committing to ${activeChangesRepo.name}...`);
  try {
    await window.cat.stageAll(activeChangesRepo.localPath);
    const sha = await window.cat.commit(
      activeChangesRepo.localPath,
      message,
      authorName,
      authorEmail
    );
    setStatus(`Committed ${sha.slice(0, 7)} to ${activeChangesRepo.name}.`);

    const changes = await window.cat.getChangedFiles(activeChangesRepo.localPath);
    renderChangesList(changes);
    commitMessageEl.value = "";
  } catch (err) {
    setStatus(`Commit failed: ${err.message}`, true);
  }
}

commitBtn.addEventListener("click", handleCommit);
changesCloseBtn.addEventListener("click", () => {
  changesPanelEl.style.display = "none";
  activeChangesRepo = null;
});

async function handlePull(repo) {
  const passphrase = await ensurePassphrase();
  if (!passphrase) {
    setStatus("Pull cancelled — passphrase required.");
    return;
  }

  const profile = getActiveProfile(currentState);
  const authorName = profile ? profile.userName : "Cat User";
  const authorEmail = profile ? profile.userEmail : "cat@local";

  setStatus(`Pulling ${repo.name}...`);
  try {
    const output = await window.cat.pull(
      repo.localPath,
      authorName,
      authorEmail,
      currentState.activeProfile,
      passphrase
    );
    setStatus(`Pulled ${repo.name}:\n${output}`);
  } catch (err) {
    if (err.message.includes("decrypt")) resetPassphrase();
    setStatus(`Pull failed for ${repo.name}: ${err.message}`, true);
  }
}

async function handlePush(repo) {
  const passphrase = await ensurePassphrase();
  if (!passphrase) {
    setStatus("Push cancelled — passphrase required.");
    return;
  }

  setStatus(`Pushing ${repo.name}...`);
  try {
    const output = await window.cat.push(repo.localPath, currentState.activeProfile, passphrase);
    setStatus(`Pushed ${repo.name}:\n${output}`);
  } catch (err) {
    if (err.message.includes("decrypt")) resetPassphrase();
    setStatus(`Push failed for ${repo.name}: ${err.message}`, true);
  }
}

async function handleClone() {
  const remoteUrl = cloneUrlInput.value.trim();
  if (!remoteUrl) return;

  const targetDir = repoDirName(remoteUrl);
  setStatus(`Cloning ${remoteUrl}...`);

  const passphrase = currentState.activeProfile ? await ensurePassphrase() : null;
  if (currentState.activeProfile && !passphrase) {
    setStatus("Clone cancelled — passphrase required to authenticate.");
    return;
  }

  try {
    await window.cat.cloneRepo(remoteUrl, targetDir, currentState.activeProfile, passphrase);

    const profile = getActiveProfile(currentState);
    if (profile) {
      await window.cat.setRepoIdentity(targetDir, profile.userName, profile.userEmail);
    }

    currentState.repos.push({ name: targetDir, remoteUrl, localPath: targetDir });
    await window.cat.saveState(currentState);

    cloneUrlInput.value = "";
    setStatus(
      profile
        ? `Cloned ${targetDir} as ${profile.name}.`
        : `Cloned ${targetDir} (no active profile set).`
    );
    renderRepoList(currentState);
  } catch (err) {
    if (err.message.includes("decrypt")) resetPassphrase();
    setStatus(`Clone failed: ${err.message}`, true);
  }
}

cloneBtn.addEventListener("click", handleClone);

const checkUpdateBtn = document.getElementById("check-update-btn");
const updateStatusEl = document.getElementById("update-status");

async function checkForUpdate({ silent } = { silent: false }) {
  updateStatusEl.textContent = "Checking...";
  try {
    const manifest = await window.cat.checkForUpdate();

    if (!manifest) {
      updateStatusEl.textContent = silent ? "" : "Cat is up to date.";
      return;
    }

    const confirmed = await showConfirmModal(
      `A new version of Cat is available (v${manifest.version}). Download it now? ` +
      `It will be applied automatically the next time Cat is launched.`
    );

    if (!confirmed) {
      updateStatusEl.textContent = `Update v${manifest.version} available (deferred).`;
      return;
    }

    updateStatusEl.textContent = `Downloading v${manifest.version}...`;
    await window.cat.downloadUpdate(manifest);
    updateStatusEl.textContent = `Update v${manifest.version} downloaded — restart Cat to apply.`;
  } catch (err) {
    updateStatusEl.textContent = silent ? "" : `Update check failed: ${err.message}`;
  }
}

checkUpdateBtn.addEventListener("click", () => checkForUpdate({ silent: false }));

document.title = "Cat v" + window.cat.version;
loadState();
checkForUpdate({ silent: true }); // quiet check on launch, no popup if already current