const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { open } = window.__TAURI__.dialog;

const readyPanel = document.getElementById("ready");
const settingsPanel = document.getElementById("settings");
const activePanel = document.getElementById("active");
const expiredPanel = document.getElementById("expired");
const summaryPanel = document.getElementById("summary");

const archiveRoot = document.getElementById("archive-root");
const exportRoot = document.getElementById("export-root");
const readyError = document.getElementById("ready-error");

const stepLabel = document.getElementById("step-label");
const rawPath = document.getElementById("raw-path");
const expected = document.getElementById("expected");
const challengeText = document.getElementById("challenge-text");
const challengeTextExpired = document.getElementById("challenge-text-expired");
const timer = document.getElementById("timer");
const status = document.getElementById("status");

const exportList = document.getElementById("export-list");

const pickArchiveButton = document.getElementById("pick-archive");
const pickExportButton = document.getElementById("pick-export");
const startButton = document.getElementById("start");
const openSettingsButton = document.getElementById("open-settings");
const nextButton = document.getElementById("next");
const skipButton = document.getElementById("skip");
const expiredNext = document.getElementById("expired-next");
const expiredSkip = document.getElementById("expired-skip");
const keepWorking = document.getElementById("keep-working");
const openFolder = document.getElementById("open-folder");
const newSession = document.getElementById("new-session");

const photosPerSessionInput = document.getElementById("photos-per-session");
const secondsPerPhotoInput = document.getElementById("seconds-per-photo");
const challengeListEl = document.getElementById("challenge-list");
const newChallengeText = document.getElementById("new-challenge-text");
const addChallengeButton = document.getElementById("add-challenge");
const settingsError = document.getElementById("settings-error");
const settingsSaveButton = document.getElementById("settings-save");
const settingsCancelButton = document.getElementById("settings-cancel");

let awaitingSuccess = false;
let archivePath = null;
let exportPath = null;
let settingsDraft = null;

const updateStartEnabled = () => {
  startButton.disabled = !(archivePath && exportPath);
};

pickArchiveButton.addEventListener("click", async () => {
  const selection = await open({ directory: true, multiple: false, title: "Choose archive folder" });
  if (selection) {
    archivePath = selection;
    archiveRoot.textContent = archivePath;
    updateStartEnabled();
  }
});

pickExportButton.addEventListener("click", async () => {
  const selection = await open({ directory: true, multiple: false, title: "Choose export folder" });
  if (selection) {
    exportPath = selection;
    exportRoot.textContent = exportPath;
    updateStartEnabled();
  }
});

const setPanel = (panel) => {
  [readyPanel, settingsPanel, activePanel, expiredPanel, summaryPanel].forEach((item) => {
    item.classList.remove("active");
  });
  panel.classList.add("active");
};

const renderChallengeList = () => {
  challengeListEl.innerHTML = "";
  settingsDraft.challenges.forEach((challenge, index) => {
    const row = document.createElement("div");
    row.className = "challenge-row";

    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.checked = challenge.enabled;
    checkbox.addEventListener("change", () => {
      settingsDraft.challenges[index].enabled = checkbox.checked;
    });

    const text = document.createElement("span");
    text.className = "challenge-row-text";
    text.textContent = challenge.text;

    row.appendChild(checkbox);
    row.appendChild(text);

    if (challenge.custom) {
      const removeButton = document.createElement("button");
      removeButton.className = "remove-challenge";
      removeButton.textContent = "✕";
      removeButton.addEventListener("click", () => {
        settingsDraft.challenges.splice(index, 1);
        renderChallengeList();
      });
      row.appendChild(removeButton);
    }

    challengeListEl.appendChild(row);
  });
};

openSettingsButton.addEventListener("click", async () => {
  settingsError.textContent = "";
  const loaded = await invoke("get_settings");
  settingsDraft = JSON.parse(JSON.stringify(loaded));
  photosPerSessionInput.value = settingsDraft.photos_per_session;
  secondsPerPhotoInput.value = settingsDraft.seconds_per_photo;
  newChallengeText.value = "";
  renderChallengeList();
  setPanel(settingsPanel);
});

addChallengeButton.addEventListener("click", () => {
  const text = newChallengeText.value.trim();
  if (!text) {
    return;
  }
  settingsDraft.challenges.push({
    id: `custom-${Date.now()}`,
    text,
    enabled: true,
    custom: true,
  });
  newChallengeText.value = "";
  renderChallengeList();
});

settingsSaveButton.addEventListener("click", async () => {
  settingsError.textContent = "";
  settingsDraft.photos_per_session = parseInt(photosPerSessionInput.value, 10) || 1;
  settingsDraft.seconds_per_photo = parseInt(secondsPerPhotoInput.value, 10) || 5;
  try {
    await invoke("save_settings", { settings: settingsDraft });
    setPanel(readyPanel);
  } catch (error) {
    settingsError.textContent = String(error);
  }
});

settingsCancelButton.addEventListener("click", () => {
  setPanel(readyPanel);
});

const setStatus = (text) => {
  status.textContent = text;
};

const setTimer = (remaining) => {
  const minutes = Math.floor(remaining / 60);
  const seconds = remaining % 60;
  timer.textContent = `${minutes}:${seconds.toString().padStart(2, "0")}`;
};

startButton.addEventListener("click", async () => {
  readyError.textContent = "";
  startButton.textContent = "Start";
  setPanel(activePanel);
  setStatus("Starting session...");
  awaitingSuccess = false;
  try {
    await invoke("start_session", { archiveRoot: archivePath, exportRoot: exportPath });
  } catch (error) {
    readyError.textContent = String(error);
    setPanel(readyPanel);
  }
});

nextButton.addEventListener("click", async () => {
  awaitingSuccess = false;
  await invoke("manual_next");
});

skipButton.addEventListener("click", async () => {
  awaitingSuccess = false;
  await invoke("skip_step");
});

expiredNext.addEventListener("click", async () => {
  awaitingSuccess = false;
  await invoke("manual_next");
  setPanel(activePanel);
});

expiredSkip.addEventListener("click", async () => {
  awaitingSuccess = false;
  await invoke("skip_step");
  setPanel(activePanel);
});

keepWorking.addEventListener("click", async () => {
  await invoke("keep_working");
  setPanel(activePanel);
});

openFolder.addEventListener("click", async () => {
  await invoke("open_export_folder");
});

newSession.addEventListener("click", async () => {
  readyError.textContent = "";
  startButton.textContent = "Start";
  await invoke("start_session", { archiveRoot: archivePath, exportRoot: exportPath });
  setPanel(activePanel);
});

listen("session_error", (event) => {
  readyError.textContent = event.payload.message;
  startButton.textContent = "Restart";
  setPanel(readyPanel);
});

listen("step_started", (event) => {
  awaitingSuccess = false;
  const payload = event.payload;
  setPanel(activePanel);
  stepLabel.textContent = `Photo ${payload.step_index} of ${payload.total_steps}`;
  rawPath.textContent = payload.raw_path;
  expected.textContent = `Expecting ${payload.expected_jpg}`;
  challengeText.textContent = payload.challenge;
  challengeTextExpired.textContent = payload.challenge;
  setTimer(payload.seconds_remaining);
  setStatus("Waiting for export...");
});

listen("timer_tick", (event) => {
  setTimer(event.payload);
});

listen("time_expired", () => {
  setPanel(expiredPanel);
});

listen("step_resumed", () => {
  setPanel(activePanel);
});

listen("export_detected", () => {
  if (awaitingSuccess) {
    return;
  }
  awaitingSuccess = true;
  setStatus("Export detected — success");
});

listen("session_finished", (event) => {
  const payload = event.payload;
  exportList.innerHTML = "";
  if (payload.exports.length === 0) {
    const item = document.createElement("li");
    item.textContent = "No JPGs found in export folder.";
    exportList.appendChild(item);
  } else {
    payload.exports.forEach((name) => {
      const item = document.createElement("li");
      item.textContent = name;
      exportList.appendChild(item);
    });
  }
  setPanel(summaryPanel);
});

(async () => {
  const loaded = await invoke("get_settings");
  if (loaded.last_archive_root) {
    archivePath = loaded.last_archive_root;
    archiveRoot.textContent = archivePath;
  }
  if (loaded.last_export_root) {
    exportPath = loaded.last_export_root;
    exportRoot.textContent = exportPath;
  }
  updateStartEnabled();
})();

setPanel(readyPanel);
