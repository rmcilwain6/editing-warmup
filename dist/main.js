const { invoke } = window.__TAURI__.tauri;
const { listen } = window.__TAURI__.event;

const readyPanel = document.getElementById("ready");
const activePanel = document.getElementById("active");
const expiredPanel = document.getElementById("expired");
const summaryPanel = document.getElementById("summary");

const archiveRoot = document.getElementById("archive-root");
const exportRoot = document.getElementById("export-root");
const readyError = document.getElementById("ready-error");

const stepLabel = document.getElementById("step-label");
const rawPath = document.getElementById("raw-path");
const expected = document.getElementById("expected");
const timer = document.getElementById("timer");
const status = document.getElementById("status");

const exportList = document.getElementById("export-list");

const startButton = document.getElementById("start");
const nextButton = document.getElementById("next");
const skipButton = document.getElementById("skip");
const expiredNext = document.getElementById("expired-next");
const expiredSkip = document.getElementById("expired-skip");
const keepWorking = document.getElementById("keep-working");
const openFolder = document.getElementById("open-folder");
const newSession = document.getElementById("new-session");

let awaitingSuccess = false;

const setPanel = (panel) => {
  [readyPanel, activePanel, expiredPanel, summaryPanel].forEach((item) => {
    item.classList.remove("active");
  });
  panel.classList.add("active");
};

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
    await invoke("start_session");
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
  await invoke("start_session");
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

const init = async () => {
  const [archive, exportDir] = await invoke("get_config");
  archiveRoot.textContent = archive;
  exportRoot.textContent = exportDir;
  setPanel(readyPanel);
};

init();
