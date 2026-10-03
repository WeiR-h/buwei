"""Build a three-minute evidence presentation from reviewed native captures.

This is an explicitly labelled screenshot sequence, not continuous UI footage.
No generated UI or fictitious service result is used. Requires Pillow/FFmpeg
only to prepare release media.
"""
import argparse,hashlib,json,pathlib,subprocess
from PIL import Image,ImageDraw,ImageFont

def build(a):
    records=json.loads(a.captures.read_text('utf8'));formal=json.loads(a.formal.read_text('utf8'))
    version=formal['version'];community=tuple(map(int,version.split('.'))) >= (0,2,0)
    assert formal['passed'] and len(formal['formal_rounds'])==5
    assert len(records)==5 and all(r.get('manually_reviewed') for r in records)
    for file in [a.recovery,a.outages,a.original_editor]:
        evidence=json.loads(file.read_text('utf8'))
        assert evidence['passed'] and evidence['version']==version
    assert len(json.loads(a.recovery.read_text('utf8'))['cases'])==6
    assert len(json.loads(a.outages.read_text('utf8'))['cases'])==3
    a.directory.mkdir(parents=True,exist_ok=False)
    font=ImageFont.truetype(str(a.font),34);small=ImageFont.truetype(str(a.font),25);big=ImageFont.truetype(str(a.font),66)
    slides=[]
    def make(title,lines,seconds,photo=None):
        canvas=Image.new('RGB',(1920,1080),'#173c32');d=ImageDraw.Draw(canvas)
        d.text((70,40),title,font=font,fill='white')
        if photo:
            image=Image.open(photo).convert('RGB');image.thumbnail((1760,890));canvas.paste(image,((1920-image.width)//2,120))
        else:
            for n,line in enumerate(lines):d.text((100,240+n*100),line,font=big if n==0 else font,fill='white' if n==0 else '#c4ded3')
        d.text((70,1030),f'补位 v{version} · 实际原生截图序列 · 两个独立账号 · 私密字段遮盖',font=small,fill='#c4ded3')
        file=a.directory/f'{len(slides):02d}.png';canvas.save(file);slides.append({'file':file.name,'duration_seconds':seconds,'sha256':hashlib.sha256(file.read_bytes()).hexdigest()})
    make('补位 BuWei · 让想来的人，刚好有位。',['生生不息 · WeiR-h',('最多五场活动 · 每场 30 个名额 · Windows 官方宿主' if community else '一场活动 · 最多 30 人 · Windows 官方宿主'),'确认后执行，服务端核实后显示成功。'],20)
    titles=['本人报名 → 候补队列',('规则确认 → 自动保留与邀请' if community else '组织者确认邀请 → 保留与送达'),'参与者本人收到邀请','本人接受 → 组织者核验结果','本人取消 → 释放名额']
    for item,title,seconds in zip(records,titles,[20,20,20,20,20]):
        photo=a.captures.parent/item['file'];assert hashlib.sha256(photo.read_bytes()).hexdigest()==item['published_sha256']
        make(title,[],seconds,photo)
    make('确认、执行与回执恢复',['五轮真实双账号闭环','报名、邀请、接受、核验、取消和释放；另测拒绝与五分钟过期。','丢失回执与崩溃：沿原编号恢复，服务端各只有一条事件。','实际发送前断网：身份核验阻止执行，旧确认失效。'],25)
    make('补位与文章共用 action-receipts',['prepare → confirm → execute → reconcile → receipt','MiniMax-M3 只形成建议与草稿，经规则检查和本人确认。','原 Rinx 编辑器的保存、全文预览、发布和应用分享另行回归。',('活动卡片报名、同行整组占位；授权后自动读取回复并递补。' if community else '应用打开且授权有效时同步回复。')],20)
    make('开始组织下一场活动',['github.com/WeiR-h/buwei','Apache-2.0 · 固定官方依赖 · SHA-256 · Windows 两角色入口','登录 Rinx，核对本人权限，创建并分享第一场活动。'],15)
    assert sum(s['duration_seconds'] for s in slides)==180
    concat=a.directory/'frames.ffconcat';concat.write_text('ffconcat version 1.0\n'+''.join(f"file '{s['file']}'\nduration {s['duration_seconds']}\n" for s in slides)+f"file '{slides[-1]['file']}'\n",'utf8')
    subprocess.run([str(a.ffmpeg),'-hide_banner','-loglevel','error','-f','concat','-safe','1','-i',str(concat),'-t','180','-vf','fps=10,format=yuv420p','-c:v','libx264','-preset','medium','-crf','21','-movflags','+faststart',str(a.output)],check=True)
    report={'version':version,'passed':True,'duration_seconds':180,'presentation':'reviewed actual native screenshot sequence; real SDK flow driven by explicitly opted-in acceptance controller; not continuous screen recording','private_fields_masked':True,'all_selected_captures_manually_reviewed':True,'video_sha256':hashlib.sha256(a.output.read_bytes()).hexdigest(),'slides':slides}
    (a.directory/'video.json').write_text(json.dumps(report,indent=2),'utf8');return report

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--captures',type=pathlib.Path,required=True);p.add_argument('--formal',type=pathlib.Path,required=True);p.add_argument('--recovery',type=pathlib.Path,required=True);p.add_argument('--outages',type=pathlib.Path,required=True);p.add_argument('--original-editor',type=pathlib.Path,required=True);p.add_argument('--directory',type=pathlib.Path,required=True);p.add_argument('--font',type=pathlib.Path,required=True);p.add_argument('--ffmpeg',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);print(json.dumps(build(p.parse_args())))
