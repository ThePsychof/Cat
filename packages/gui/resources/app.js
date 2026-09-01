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
  
  // Enhance status messages with cat emoji for operations
  if (text.includes("Loading") || text.includes("Pulling") || text.includes("Pushing") || 
      text.includes("Cloning") || text.includes("Switching") || text.includes("Creating") ||
      text.includes("Committing") || text.includes("Downloading")) {
    statusEl.textContent = "🐈 " + text;
  }
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
  const repoViewerEl = document.getElementById("repo-viewer");
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
    label.style.cursor = "pointer";
    label.onclick = () => selectRepositoryFromList(repo);

    const actions = document.createElement("span");
    actions.className = "repo-actions";

    const changesBtn = document.createElement("button");
    changesBtn.textContent = "Changes";
    changesBtn.onclick = (e) => {
      e.stopPropagation();
      openChangesPanel(repo);
    };

    const pullBtn = document.createElement("button");
    pullBtn.textContent = "Pull";
    pullBtn.onclick = (e) => {
      e.stopPropagation();
      handlePull(repo);
    };

    const pushBtn = document.createElement("button");
    pushBtn.textContent = "Push";
    pushBtn.onclick = (e) => {
      e.stopPropagation();
      handlePush(repo);
    };

    actions.appendChild(changesBtn);
    actions.appendChild(pullBtn);
    actions.appendChild(pushBtn);

    li.appendChild(label);
    li.appendChild(actions);
    repoListEl.appendChild(li);
  }
}

async function selectRepositoryFromList(repo) {
  activeChangesRepo = repo;
  const repoViewerEl = document.getElementById("repo-viewer");
  const emptyStateEl = document.getElementById("empty-state");
  
  if (repoViewerEl && emptyStateEl) {
    emptyStateEl.style.display = "none";
    repoViewerEl.style.display = "block";
  }
  
  const repoNameEl = document.getElementById("repo-name");
  const remoteUrlEl = document.getElementById("remote-url");
  
  if (repoNameEl) repoNameEl.textContent = repo.name;
  if (remoteUrlEl) remoteUrlEl.textContent = repo.remoteUrl || "No remote";
  
  try {
    await refreshBranches(repo);
    const changes = await window.cat.getChangedFiles(repo.localPath);
    renderChangesList(changes);
    await loadFileTree(repo);
    await loadCommitLog(repo);
  } catch (err) {
    setStatus(`Failed to load repo details: ${err.message}`, true);
  }
  
  switchRepoTab("status");
}

const changesFileListEl = document.getElementById("changes-file-list");
const commitMessageEl = document.getElementById("commit-message");
const commitBtn = document.getElementById("commit-btn");

let activeChangesRepo = null;

const branchSelectEl = document.getElementById("branch-select");
const newBranchBtn = document.getElementById("new-branch-btn");

async function openChangesPanel(repo) {
  activeChangesRepo = repo;
  
  // Switch to status tab to show changes
  selectRepositoryFromList(repo);
  
  try {
    const changes = await window.cat.getChangedFiles(repo.localPath);
    renderChangesList(changes);
  } catch (err) {
    setStatus(`Failed to get changes for ${repo.name}: ${err.message}`, true);
  }

  try {
    await refreshBranches(repo);
  } catch (err) {
    setStatus(`Failed to load branches for ${repo.name}: ${err.message}`, true);
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
    li.textContent = "✓ No changes — working directory is clean";
    li.style.color = "var(--text-muted)";
    li.style.padding = "8px";
    changesFileListEl.appendChild(li);
    return;
  }

  for (const change of changes) {
    const li = document.createElement("li");
    li.style.display = "flex";
    li.style.justifyContent = "space-between";
    li.style.padding = "6px 0";
    li.style.borderBottom = "1px solid var(--line)";

    // Status icon based on change type
    let icon = "•";
    let statusText = change.status;
    
    if (change.status === "modified") {
      icon = "⚙";
    } else if (change.status === "added") {
      icon = "🟢";
    } else if (change.status === "deleted") {
      icon = "🔴";
    } else if (change.status === "renamed") {
      icon = "➜";
    } else if (change.status === "untracked") {
      icon = "?";
    }

    const name = document.createElement("span");
    name.textContent = `${icon} ${change.filepath}`;
    name.style.flex = "1";

    const tag = document.createElement("span");
    tag.className = "status-tag";
    tag.textContent = statusText;
    tag.style.color = "var(--ember)";
    tag.style.fontSize = "10px";
    tag.style.textTransform = "uppercase";
    tag.style.letterSpacing = "0.05em";

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

// Remove the old changes close handler since we're integrating it into the repo viewer
// changesCloseBtn is no longer needed in the new layout

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

async function handleOpenEditor(repo) {
  if (!repo) return;
  setStatus(`Opening ${repo.name} in external editor...`);
  try {
    await window.cat.openInEditor(repo.localPath);
    setStatus(`Opened ${repo.name} in your editor.`);
  } catch (err) {
    setStatus(`Open failed: ${err.message}`, true);
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

document.addEventListener("cat-ready", async () => {
  const v = await window.cat.version();
  document.title = "Cat v" + v;
  loadState();
  checkForUpdate({ silent: true }); // quiet check on launch, no popup if already current
  
  document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.addEventListener('click', (e) => {
      const tabName = e.target.dataset.tab;
      if (tabName) switchRepoTab(tabName);
    });
  });

  const openBtn = document.getElementById('open-btn');
  if (openBtn) {
    openBtn.addEventListener('click', () => {
      if (activeChangesRepo) handleOpenEditor(activeChangesRepo);
    });
  }
});

// Repository browser tab switching
function switchRepoTab(tabName) {
  // Hide all tabs
  document.querySelectorAll('.tab-content').forEach(tab => {
    tab.classList.remove('active');
  });
  
  // Remove active class from all buttons
  document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.classList.remove('active');
  });
  
  // Show selected tab
  const tab = document.getElementById(`${tabName}-tab`);
  if (tab) {
    tab.classList.add('active');
  }
  
  // Mark button as active
  const btn = document.querySelector(`[data-tab="${tabName}"]`);
  if (btn) {
    btn.classList.add('active');
  }

  // Load content when switching tabs
  if (activeChangesRepo) {
    if (tabName === 'files') {
      loadFileTree(activeChangesRepo);
    } else if (tabName === 'commits') {
      loadCommitLog(activeChangesRepo);
    }
  }
}

async function loadFileTree(repo) {
  const fileTreeEl = document.getElementById('file-tree');
  if (!fileTreeEl) return;

  fileTreeEl.innerHTML = '<div style="padding: 8px; color: var(--text-muted);">🐈 Loading files...</div>';

  try {
    const files = await window.cat.listFiles(repo.localPath);
    fileTreeEl.innerHTML = '';
    renderFileTree(fileTreeEl, files);
  } catch (err) {
    fileTreeEl.innerHTML = `<div style="padding: 8px; color: var(--ember);">❌ ${err.message}</div>`;
  }
}

function renderFileTree(container, files) {
  const tree = {};
  
  files.forEach(filepath => {
    const parts = filepath.split('/');
    let current = tree;
    
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      if (!current[part]) {
        current[part] = {};
      }
      current = current[part];
    }
  });

  const renderNode = (node, path = '', depth = 0) => {
    const list = document.createElement('ul');
    list.style.listStyle = 'none';
    list.style.paddingLeft = depth > 0 ? '16px' : '0';
    list.style.margin = '0';

    Object.keys(node).sort().forEach(key => {
      const li = document.createElement('li');
      const isDir = Object.keys(node[key]).length > 0;
      const icon = isDir ? '📁' : '📄';
      
      const span = document.createElement('span');
      span.textContent = `${icon} ${key}`;
      span.style.padding = '2px 4px';
      span.style.display = 'block';
      
      li.appendChild(span);
      if (isDir) {
        li.appendChild(renderNode(node[key], path + '/' + key, depth + 1));
      }
      list.appendChild(li);
    });

    return list;
  };

  container.innerHTML = '';
  container.appendChild(renderNode(tree));
}

async function loadCommitLog(repo) {
  const commitListEl = document.getElementById('commit-list');
  if (!commitListEl) return;

  commitListEl.innerHTML = '<div style="padding: 8px; color: var(--text-muted);">🐈 Loading commits...</div>';

  try {
    const commits = await window.cat.getCommitLog(repo.localPath, 20);
    commitListEl.innerHTML = '';
    renderCommitList(commitListEl, commits);
  } catch (err) {
    commitListEl.innerHTML = `<div style="padding: 8px; color: var(--ember);">❌ ${err.message}</div>`;
  }
}

function renderCommitList(container, commits) {
  const list = document.createElement('ul');
  list.style.listStyle = 'none';
  list.style.padding = '0';
  list.style.margin = '0';

  commits.forEach((commit, idx) => {
    if (idx > 0) {
      const divider = document.createElement('li');
      divider.style.borderBottom = '1px solid var(--line)';
      divider.style.margin = '8px 0';
      list.appendChild(divider);
    }

    const li = document.createElement('li');
    li.style.padding = '8px';
    li.style.cursor = 'pointer';
    li.style.borderRadius = '4px';
    li.style.transition = 'background-color 0.15s ease';
    
    li.onmouseover = () => li.style.backgroundColor = 'var(--line)';
    li.onmouseout = () => li.style.backgroundColor = 'transparent';

    const shaEl = document.createElement('code');
    shaEl.textContent = commit.sha.slice(0, 7);
    shaEl.style.color = 'var(--ember)';
    shaEl.style.fontWeight = 'bold';

    const authorEl = document.createElement('div');
    authorEl.style.color = 'var(--text-muted)';
    authorEl.style.fontSize = '11px';
    authorEl.style.marginTop = '2px';
    authorEl.textContent = `${commit.author} · ${commit.date}`;

    const messageEl = document.createElement('div');
    messageEl.textContent = commit.message;
    messageEl.style.marginTop = '4px';

    li.appendChild(shaEl);
    li.appendChild(authorEl);
    li.appendChild(messageEl);
    list.appendChild(li);
  });

  container.appendChild(list);
}

// Orange cat animations and helpers
const catAnimations = {
  walking: () => `
    <svg viewBox="0 0 120 80" style="width: 32px; height: 32px; animation: catWalk 1s linear infinite;">
      <ellipse cx="60" cy="55" rx="46" ry="20" fill="#2c2724"/>
      <path d="M20 50 Q10 20 35 25 Q45 10 60 22 Q75 10 85 25 Q110 20 100 50 Q100 68 60 68 Q20 68 20 50Z" fill="#f2661a"/>
      <path d="M32 26 Q28 30 34 34" stroke="#0f0e0d" stroke-width="2" fill="none" stroke-linecap="round"/>
      <path d="M88 26 Q92 30 86 34" stroke="#0f0e0d" stroke-width="2" fill="none" stroke-linecap="round"/>
    </svg>
  `,
  loading: () => `
    <svg viewBox="0 0 120 80" style="width: 24px; height: 24px; animation: spin 2s linear infinite;">
      <circle cx="60" cy="40" r="8" fill="#f2661a" opacity="0.3"/>
      <circle cx="60" cy="40" r="8" fill="none" stroke="#f2661a" stroke-width="2" stroke-dasharray="12 16" stroke-linecap="round"/>
    </svg>
  `,
  success: () => `
    <svg viewBox="0 0 120 80" style="width: 24px; height: 24px;">
      <path d="M60 50 l-10 10 l20 20 l40-50" stroke="#f2661a" stroke-width="3" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
  `,
};

// Add CSS for cat animations
if (!document.getElementById('cat-animations-css')) {
  const style = document.createElement('style');
  style.id = 'cat-animations-css';
  style.textContent = `
    @keyframes catWalk {
      0%, 100% { transform: translateX(0) scaleX(1); }
      50% { transform: translateX(4px) scaleX(-1); }
    }
    @keyframes spin {
      0% { transform: rotate(0deg); }
      100% { transform: rotate(360deg); }
    }
  `;
  document.head.appendChild(style);
}