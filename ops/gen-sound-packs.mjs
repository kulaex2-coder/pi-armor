import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = join("E:/workspace/pi-armor/src-tauri/resources/sounds");

function writeWav(path, samples, sampleRate = 22050) {
  const dataSize = samples.length * 2;
  const buf = Buffer.alloc(44 + dataSize);
  buf.write("RIFF", 0);
  buf.writeUInt32LE(36 + dataSize, 4);
  buf.write("WAVE", 8);
  buf.write("fmt ", 12);
  buf.writeUInt32LE(16, 16);
  buf.writeUInt16LE(1, 20);
  buf.writeUInt16LE(1, 22);
  buf.writeUInt32LE(sampleRate, 24);
  buf.writeUInt32LE(sampleRate * 2, 28);
  buf.writeUInt16LE(2, 32);
  buf.writeUInt16LE(16, 34);
  buf.write("data", 36);
  buf.writeUInt32LE(dataSize, 40);
  for (let i = 0; i < samples.length; i++) {
    const v = Math.max(-1, Math.min(1, samples[i]));
    buf.writeInt16LE((v * 32767) | 0, 44 + i * 2);
  }
  writeFileSync(path, buf);
}

function tone(freq, ms, sampleRate, gain = 0.28) {
  const n = Math.floor((sampleRate * ms) / 1000);
  const out = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    const t = i / sampleRate;
    const env = Math.min(1, i / (sampleRate * 0.01)) * Math.exp(-3.2 * t);
    out[i] = Math.sin(2 * Math.PI * freq * t) * gain * env;
  }
  return out;
}

function concat(...parts) {
  let len = 0;
  for (const p of parts) len += p.length;
  const out = new Float64Array(len);
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

function silence(ms, sampleRate = 22050) {
  return new Float64Array(Math.floor((sampleRate * ms) / 1000));
}

const packs = [
  {
    id: "soft-chime",
    title: "柔和双音",
    samples: concat(tone(1046.5, 180), silence(40), tone(1318.5, 260)),
  },
  {
    id: "bright-ding",
    title: "清脆单响",
    samples: tone(1760, 260, 22050, 0.32),
  },
  {
    id: "deep-thud",
    title: "低沉一击",
    samples: tone(220, 320, 22050, 0.4),
  },
  {
    id: "rising-success",
    title: "上行三音",
    samples: concat(tone(784, 140), silence(30), tone(988, 140), silence(30), tone(1175, 220)),
  },
  {
    id: "glass-tap",
    title: "玻璃轻点",
    samples: tone(2093, 180, 22050, 0.22),
  },
  {
    id: "wood-click",
    title: "木击短音",
    samples: tone(660, 120, 22050, 0.35),
  },
];

const catalog = [];
for (const pack of packs) {
  const dir = join(root, pack.id);
  mkdirSync(dir, { recursive: true });
  writeWav(join(dir, "done.wav"), pack.samples);
  catalog.push({ id: pack.id, title: pack.title, file: "done.wav" });
}
writeFileSync(join(root, "catalog.json"), JSON.stringify(catalog, null, 2) + "\n");
console.log("packs", catalog.length);
