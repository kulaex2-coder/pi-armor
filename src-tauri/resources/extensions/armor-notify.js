/**
 * Pi Armor notify — play Edge/local WAV prompts on Pi lifecycle events.
 * Config: ~/.pi/agent/armor-notify.json
 * Sounds: config.dir/*.wav (file name = event id)
 */
import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { homedir, platform } from "node:os";
import { join } from "node:path";

const CONFIG_PATH = join(homedir(), ".pi", "agent", "armor-notify.json");
const MIN_INTERVAL_MS = 400;
let lastKey = "";
let lastAt = 0;

function loadConfig() {
  try {
    const raw = JSON.parse(readFileSync(CONFIG_PATH, "utf8"));
    return {
      enabled: raw.enabled !== false,
      dir: typeof raw.dir === "string" ? raw.dir : "",
      packId: typeof raw.packId === "string" ? raw.packId : "",
    };
  } catch {
    return { enabled: false, dir: "", packId: "" };
  }
}

function emitBell() {
  try {
    process.stdout.write("\x07");
  } catch {
    // ignore
  }
}

function spawnQuiet(command, args) {
  try {
    const child = spawn(command, args, {
      stdio: "ignore",
      windowsHide: true,
      detached: true,
    });
    child.unref();
  } catch {
    emitBell();
  }
}

function playWav(path) {
  if (!path || !existsSync(path)) {
    emitBell();
    return;
  }
  const os = platform();
  if (os === "win32") {
    const escaped = path.replace(/'/g, "''");
    spawnQuiet("powershell.exe", [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `(New-Object System.Media.SoundPlayer '${escaped}').PlaySync()`,
    ]);
    return;
  }
  if (os === "darwin") {
    spawnQuiet("afplay", [path]);
    return;
  }
  spawnQuiet("sh", ["-c", 'paplay "$1" 2>/dev/null || aplay -q "$1"', "sh", path]);
}

function playEvent(eventId) {
  const cfg = loadConfig();
  if (!cfg.enabled || !cfg.dir) return;
  const now = Date.now();
  const key = eventId;
  if (key === lastKey && now - lastAt < MIN_INTERVAL_MS) return;
  lastKey = key;
  lastAt = now;
  playWav(join(cfg.dir, `${eventId}.wav`));
}

export default function (pi) {
  pi.on("session_start", () => playEvent("session_start"));
  pi.on("session_shutdown", () => playEvent("session_shutdown"));
  pi.on("agent_start", () => playEvent("agent_start"));
  pi.on("agent_settled", (event) => {
    if (event && event.aborted) playEvent("agent_aborted");
    else playEvent("agent_settled");
  });
  pi.on("turn_start", () => playEvent("turn_start"));
  pi.on("turn_end", () => playEvent("turn_end"));
  pi.on("tool_call", () => playEvent("tool_call"));
  pi.on("tool_result", () => playEvent("tool_result"));
  pi.on("tool_execution_end", (event) => {
    if (event && event.isError) playEvent("tool_error");
  });
  pi.on("ui_prompt_start", () => playEvent("ui_prompt_start"));
  pi.on("session_before_compact", () => playEvent("session_before_compact"));
}
