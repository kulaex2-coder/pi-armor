# -*- coding: utf-8 -*-
"""Generate Edge TTS prompts aligned with Pi lifecycle events."""
from __future__ import annotations

import asyncio
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(r"E:\workspace\pi-armor")
OUT = ROOT / "src-tauri" / "resources" / "sounds" / "edge-zh-xiaoxiao"
VOICE = "zh-CN-XiaoxiaoNeural"
EDGE_TTS = Path(r"C:\dev\python312\Scripts\edge-tts.exe")
PYTHON = Path(r"C:\dev\python312\python.exe")

# id == pi event key (agent_aborted is derived from agent_settled.aborted)
CLIPS = [
    ("session_start", "会话已开始"),
    ("session_shutdown", "会话已结束"),
    ("agent_start", "开始处理"),
    ("agent_settled", "任务已完成"),
    ("agent_aborted", "已中止"),
    ("turn_start", "收到新消息"),
    ("turn_end", "本轮回复完成"),
    ("tool_call", "正在调用工具"),
    ("tool_result", "工具已返回"),
    ("tool_error", "工具执行失败"),
    ("ui_prompt_start", "需要您确认"),
    ("session_before_compact", "正在压缩上下文"),
]


def ensure_deps() -> str:
    try:
        import imageio_ffmpeg  # noqa: F401
    except ImportError:
        subprocess.check_call(
            [str(PYTHON), "-m", "pip", "install", "imageio-ffmpeg", "edge-tts", "-q"]
        )
    import imageio_ffmpeg

    return imageio_ffmpeg.get_ffmpeg_exe()


async def synth(text: str, mp3: Path) -> None:
    proc = await asyncio.create_subprocess_exec(
        str(EDGE_TTS),
        "--voice",
        VOICE,
        "--text",
        text,
        "--write-media",
        str(mp3),
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    _out, err = await proc.communicate()
    if proc.returncode != 0:
        raise RuntimeError(err.decode("utf-8", "replace") or "edge-tts failed")


def to_wav(ffmpeg: str, mp3: Path, wav: Path) -> None:
    subprocess.check_call(
        [
            ffmpeg,
            "-y",
            "-i",
            str(mp3),
            "-acodec",
            "pcm_s16le",
            "-ac",
            "1",
            "-ar",
            "22050",
            str(wav),
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


async def main() -> int:
    if not EDGE_TTS.is_file():
        print("edge-tts not found:", EDGE_TTS)
        return 1
    ffmpeg = ensure_deps()
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True, exist_ok=True)
    manifest = {
        "id": "edge-zh-xiaoxiao",
        "title": "Edge 语音 · 晓晓",
        "voice": VOICE,
        "engine": "edge-tts",
        "clips": [],
    }
    for clip_id, text in CLIPS:
        mp3 = OUT / f"{clip_id}.mp3"
        wav = OUT / f"{clip_id}.wav"
        print("synth", clip_id, text)
        await synth(text, mp3)
        to_wav(ffmpeg, mp3, wav)
        mp3.unlink(missing_ok=True)
        manifest["clips"].append(
            {"id": clip_id, "text": text, "file": f"{clip_id}.wav", "event": clip_id}
        )

    # Catalog / legacy primary file for previews that expect done.wav
    shutil.copyfile(OUT / "agent_settled.wav", OUT / "done.wav")

    (OUT / "pack.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    catalog_path = ROOT / "src-tauri" / "resources" / "sounds" / "catalog.json"
    catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
    catalog = [item for item in catalog if item.get("id") != manifest["id"]]
    catalog.append(
        {
            "id": manifest["id"],
            "title": manifest["title"],
            "file": "done.wav",
            "kind": "voice",
        }
    )
    catalog_path.write_text(
        json.dumps(catalog, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print("OK", OUT)
    print("clips", len(manifest["clips"]))
    return 0


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
