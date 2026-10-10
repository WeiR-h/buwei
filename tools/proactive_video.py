"""Render a reviewed native walkthrough; no generated or reconstructed UI.

Media preparation only. The submitted evidence remains a labelled screenshot
sequence of the real SDK build, not a claim of continuous screen recording.
"""
import argparse, hashlib, json, pathlib, subprocess
from PIL import Image, ImageDraw, ImageFont


def build(args):
    formal = json.loads(args.formal.read_text('utf8'))
    records = json.loads(args.captures.read_text('utf8'))
    assert formal['passed'] and len(formal['formal_rounds']) == 5
    assert formal['proactive_detection'] and formal['personal_confirmation']
    assert len(records) == 5 and all(r.get('manually_reviewed') for r in records)
    version = formal['version']
    args.directory.mkdir(parents=True, exist_ok=False)
    font = ImageFont.truetype(str(args.font), 36)
    small = ImageFont.truetype(str(args.font), 25)
    slides = []

    def slide(title, lines, seconds, photo=None):
        canvas = Image.new('RGB', (1920, 1080), '#173c32')
        draw = ImageDraw.Draw(canvas)
        draw.text((70, 35), title, font=font, fill='white')
        if photo:
            picture = Image.open(photo).convert('RGB')
            picture.thumbnail((1780, 850))
            canvas.paste(picture, ((1920-picture.width)//2, 120))
        else:
            for i, line in enumerate(lines):
                draw.text((90, 250+i*110), line, font=font, fill='#c4ded3')
        draw.text((70, 1018), f'补位 v{version} · 原生 SDK 验收构建截图序列 · 测试账号 · 私密字段已遮盖', font=small, fill='#c4ded3')
        file = args.directory/f'{len(slides):02d}.png'
        canvas.save(file)
        slides.append({'file': file.name, 'duration_seconds': seconds,
                       'sha256': hashlib.sha256(file.read_bytes()).hexdigest()})

    slide('补位 · 记住目标，主动准备下一步', [
        '生生不息 · 唯一作者 WeiR-h',
        '组织者持续跟进人数缺口，成员发现适合自己的活动。',
        'Windows · OctoSense / Rinx / Makepad / OctoScript',
        '下面使用真实界面讲解操作，测试活动不代表实际到场。'], 20)
    for record in records:
        file = args.captures.parent/record['file']
        assert file.is_file() and hashlib.sha256(file.read_bytes()).hexdigest() == record['published_sha256']
        slide(record['title'], [], 24, file)
    slide('持续核实结果，再帮助下一次', [
        '五轮真实双账号：主动建议 → 本人确认 → 自动邀请 → 接受 → 取消。',
        '报名、邀请、回复和取消均核对服务端事件，重复事件不重复执行。',
        '模型提出建议；身份、队列、人数和授权由业务规则核验。',
        '“仅本次”与“长期偏好”分别处理，明确反馈可以撤回。'], 25)
    slide('下载与开始使用', ['github.com/WeiR-h/buwei',
        '登录自己的 Rinx 账号 → 核对权限 → 保存目标并接入活动。',
        'Apache-2.0 · 固定依赖 · 源码构建说明 · Windows 双角色入口'], 15)
    assert sum(s['duration_seconds'] for s in slides) == 180
    concat = args.directory/'frames.ffconcat'
    concat.write_text('ffconcat version 1.0\n'+''.join(
        f"file '{s['file']}'\nduration {s['duration_seconds']}\n" for s in slides
    )+f"file '{slides[-1]['file']}'\n", 'utf8')
    subprocess.run([str(args.ffmpeg), '-hide_banner', '-loglevel', 'error',
        '-f', 'concat', '-safe', '1', '-i', str(concat), '-t', '180',
        '-vf', 'fps=10,format=yuv420p', '-c:v', 'libx264', '-preset', 'medium',
        '-crf', '21', '-movflags', '+faststart', str(args.output)], check=True)
    report = {'version': version, 'passed': True, 'duration_seconds': 180,
        'presentation': 'reviewed actual native screenshot sequence; opt-in SDK test build; not continuous recording',
        'private_fields_masked': True, 'all_selected_captures_manually_reviewed': True,
        'video_sha256': hashlib.sha256(args.output.read_bytes()).hexdigest(), 'slides': slides}
    (args.directory/'video.json').write_text(json.dumps(report, indent=2), 'utf8')
    return report


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    for name in ('captures', 'formal', 'directory', 'font', 'ffmpeg', 'output'):
        p.add_argument('--'+name, type=pathlib.Path, required=True)
    print(json.dumps(build(p.parse_args())))
