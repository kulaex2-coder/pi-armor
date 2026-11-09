import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const root = "E:/workspace/pi-armor/src-tauri/resources/sounds";
const agent = join(homedir(), ".pi", "agent");
const catalog = JSON.parse(readFileSync(join(root, "catalog.json"), "utf8"));
const settings = JSON.parse(readFileSync(join(agent, "settings.json"), "utf8"));
const hasPlugin = (settings.packages || []).some((p) => String(p).includes("pi-notify-sound"));
if (!hasPlugin) {
  console.error("FAIL plugin missing");
  process.exit(1);
}

const pack = catalog.find((p) => p.id === "soft-chime") || catalog[0];
const src = join(root, pack.id, pack.file);
const destDir = join(agent, "armor-sounds", pack.id);
mkdirSync(destDir, { recursive: true });
const dest = join(destDir, "done.wav");
copyFileSync(src, dest);
writeFileSync(
  join(agent, "done-sound.json"),
  JSON.stringify({ enabled: true, sound: dest }, null, 2) + "\n",
);

const cfg = JSON.parse(readFileSync(join(agent, "done-sound.json"), "utf8"));
if (!existsSync(cfg.sound)) {
  console.error("FAIL sound file missing", cfg.sound);
  process.exit(1);
}

const escaped = cfg.sound.replace(/'/g, "''");
const play = spawnSync(
  "powershell.exe",
  ["-NoProfile", "-NonInteractive", "-Command", `(New-Object Media.SoundPlayer '${escaped}').PlaySync()`],
  { stdio: "ignore", windowsHide: true },
);
if (play.status !== 0) {
  console.error("FAIL preview play");
  process.exit(1);
}

console.log(
  JSON.stringify(
    {
      ok: true,
      plugin: true,
      pack: pack.id,
      sound: cfg.sound,
      preview: "played",
    },
    null,
    2,
  ),
);
