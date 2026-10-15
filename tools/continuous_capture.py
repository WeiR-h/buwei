"""Continuously record BuWei's own opt-in native render in real time.

Uses the pinned Makepad loopback API, never the desktop or other applications.
Private text is covered before encoding. The recorder sends no business action.
Review the complete video visually before publishing.
"""
import argparse
import hashlib
import io
import json
import pathlib
import subprocess
import time
from capture_native import PRIVATE, redaction_targets
from native_ui import NativeUI


def validate_timeline(samples, fps):
    if not 1 <= fps <= 10:
        raise ValueError("Capture frame rate invalid")
    if len(samples) < 2 or any(b <= a for a, b in zip(samples, samples[1:])):
        raise ValueError("Increasing capture times required")
    duration = samples[-1] - samples[0]
    encoded = (len(samples) - 1) / fps
    gap = max(b-a for a, b in zip(samples, samples[1:]))
    if abs(encoded-duration) > max(.75, duration*.03) or gap > max(1.5, 3/fps):
        raise ValueError(f"Capture gaps cannot be presented as continuous real-time video: captured={duration:.3f}s encoded={encoded:.3f}s maximum_gap={gap:.3f}s")
    return dict(captured_seconds=round(duration, 3), maximum_capture_gap_seconds=round(gap, 3), frames=len(samples), fps=fps)


def private_regions(before, after, window_id):
    # Include newly rendered fields if the application updates during the grab.
    return sorted({tuple(rect) for widget in before+after
                   if widget.get("w")==window_id and widget.get("v",1)!=0
                   for rect in redaction_targets(widget)})


def frame(ui, role, caption, elapsed, font_path):
    from PIL import Image, ImageDraw, ImageFont
    tree = ui.tree()
    window = ui.window(tree)
    picture = Image.open(io.BytesIO(ui.get("g", w=window["i"], raw=1))).convert("RGB")
    width, height = window["sz"]
    sx, sy = picture.width/width, picture.height/height
    draw = ImageDraw.Draw(picture)
    for x, y, w, h in private_regions(tree, ui.tree(), window["i"]):
        if y+h <= 0 or y >= height: continue
        box = (max(0, x*sx-2), max(0, y*sy-2), min(picture.width, (x+w)*sx+2), min(picture.height, (y+h)*sy+2))
        if box[0] < box[2] and box[1] < box[3]: draw.rectangle(box, fill="#e7eeea")
    picture = picture.crop((int(12*sx), int(35*sy), int((width-12)*sx), int((height-93)*sy)))
    picture.thumbnail((1570, 820))
    canvas = Image.new("RGB", (1600, 1000), "#173c32")
    canvas.paste(picture, ((1600-picture.width)//2, 70))
    draw = ImageDraw.Draw(canvas)
    heading = ImageFont.truetype(str(font_path), 29)
    small = ImageFont.truetype(str(font_path), 22)
    draw.text((24, 18), "补位 · 连续原生界面采集 · "+role, font=heading, fill="white")
    draw.text((1340, 22), f"{int(elapsed)//60:02d}:{int(elapsed)%60:02d} 实录", font=small, fill="#d9eadf")
    draw.text((24, 927), "本次活动操作" if PRIVATE.search(caption) else caption, font=heading, fill="white")
    draw.text((24, 969), "真实服务 · 本机双账号受控演示 · 私密标识已遮盖", font=small, fill="#bed8c9")
    return canvas.tobytes()


def record(args):
    if not 1 <= args.fps <= 10 or not 10 <= args.maximum_seconds <= 900:
        raise ValueError("Recording duration or frame rate invalid")
    if args.output.exists() or args.metadata.exists():
        raise ValueError("Preserve previous recordings; choose new output files")
    ports = args.ports
    if not ports or len(ports) > 2 or len(set(ports)) != len(ports) or any(not 1024 <= p <= 65535 for p in ports):
        raise ValueError("One or two isolated BuWei ports required")
    clients = {port: NativeUI(port, args.version) for port in ports}
    for client in clients.values(): client.window()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.metadata.parent.mkdir(parents=True, exist_ok=True)
    command = [str(args.ffmpeg), "-hide_banner", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", "1600x1000", "-framerate", str(args.fps), "-i", "pipe:0", "-an", "-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(args.output)]
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stderr=subprocess.PIPE)
    started = time.monotonic()
    samples = []
    chain = bytes(32)
    completed = False
    try:
        while time.monotonic()-started < args.maximum_seconds:
            control = json.loads(args.control.read_text("utf-8")) if args.control.exists() else {}
            if control.get("stop"):
                completed = control.get("completed") is True
                break
            port = control.get("active_port", ports[0])
            if port not in clients: raise ValueError("Unregistered application")
            due = started+len(samples)/args.fps
            time.sleep(max(0, due-time.monotonic()))
            elapsed = time.monotonic()-started
            pixels = frame(clients[port], "组织者" if port == ports[0] else "成员", str(control.get("caption", "等待本人操作与服务端结果"))[:60], elapsed, args.font)
            samples.append(time.monotonic()-started)
            chain = hashlib.sha256(chain+pixels).digest()
            process.stdin.write(pixels)
        process.stdin.close()
        error = process.stderr.read().decode("utf-8", errors="replace")
        if process.wait(timeout=30) != 0: raise RuntimeError("Video encoding failed: "+error[-500:])
        measured = validate_timeline(samples, args.fps)
        if not completed: raise ValueError("Recording did not reach the completed state")
        result = dict(schema=1, passed=True, source="pinned Makepad native render capture", continuous=True, real_time=True, business_actions_from_recorder=0, model_calls_from_recorder=0, redacted_before_encoding=True, manual_visual_review_required=True, application_version=args.version, frame_chain_sha256=chain.hex(), video_sha256=hashlib.sha256(args.output.read_bytes()).hexdigest(), **measured)
        args.metadata.write_text(json.dumps(result, indent=2)+"\n", encoding="utf-8")
        return result
    finally:
        if process.poll() is None:
            if not process.stdin.closed: process.stdin.close()
            try: process.wait(timeout=15)
            except subprocess.TimeoutExpired: process.terminate(); process.wait(timeout=10)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ports", nargs="+", type=int, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--ffmpeg", type=pathlib.Path, required=True)
    parser.add_argument("--font", type=pathlib.Path, required=True)
    parser.add_argument("--control", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--metadata", type=pathlib.Path, required=True)
    parser.add_argument("--fps", type=int, default=4)
    parser.add_argument("--maximum-seconds", type=int, default=600)
    print(json.dumps(record(parser.parse_args())))
