"""Capture the actual native render and explicitly cover private UI fields.

Read-only loopback RPC. No input events or business actions. Keep original
captures and full widget snapshots in an ignored private directory. Published
images must also receive visual review; this is a narrow privacy aid, not OCR.
Requires Pillow only when preparing demonstration media, not when running BuWei.
"""
import argparse,hashlib,io,json,pathlib,re,urllib.request
PRIVATE=re.compile(r'@[A-Za-z0-9]|![A-Za-z0-9]|\$[A-Za-z0-9_-]{8,}|[a-fA-F0-9]{32,}|[A-Z]:[\\/]Users[\\/]|(?:sk-|gh[pousr]_)[A-Za-z0-9_-]{20,}')

def redaction_targets(widget):
    """Cover complete private lines only in known short, unwrapped labels.

    Other widgets and unexpectedly long or scaled layouts keep the full-cover
    fallback. This cannot infer visual lines for HTML or wrapped JSON.
    """
    text=widget.get('t','');rect=widget.get('r',[0,0,0,0])
    if not text or not PRIVATE.search(text) or rect[2]<=0 or rect[3]<=0:return []
    lines=text.splitlines();expected={'participant_preview':4,'activity':6}.get(widget.get('i'))
    x,y,w,h=rect
    known=(widget.get('ty')=='Label' and expected==len(lines)
           and 8<=h/len(lines)<=24.5
           and all(len(line)*24<=w for line in lines))
    if known:
        return [(x,y+n*h/len(lines),w,h/len(lines)) for n,line in enumerate(lines) if PRIVATE.search(line)]
    return [rect]

def render_capture(raw,snap,output,caption='',font_path=None):
    from PIL import Image,ImageDraw,ImageFont
    output.parent.mkdir(parents=True,exist_ok=True)
    picture=Image.open(io.BytesIO(raw)).convert('RGB');draw=ImageDraw.Draw(picture)
    scale_x=picture.width/1400;scale_y=picture.height/900;covered=0
    for widget in snap['s']:
        for rx,ry,rw,rh in redaction_targets(widget):
            if ry>=900 or ry+rh<=0:continue
            draw.rectangle((max(0,rx*scale_x-2),max(0,ry*scale_y-2),min(picture.width,(rx+rw)*scale_x+2),min(picture.height,(ry+rh)*scale_y+2)),fill='#e7eeea');covered+=1
    # Crop shell wallpaper and dock; keep the complete visible BuWei view.
    picture=picture.crop((int(12*scale_x),int(35*scale_y),int(1388*scale_x),int(807*scale_y)))
    picture.thumbnail((1600,960))
    canvas=Image.new('RGB',(1600,picture.height+110),'#173c32');canvas.paste(picture,((1600-picture.width)//2,0))
    draw=ImageDraw.Draw(canvas)
    font=ImageFont.truetype(str(font_path),25) if font_path else ImageFont.load_default()
    draw.text((28,picture.height+18),caption,fill='white',font=font)
    draw.text((28,picture.height+58),'真实原生界面 · 账号、房间和操作编号已遮盖 · 业务由验收控制调用',fill='#b7d3c7',font=font)
    canvas.save(output)
    return {'read_only_native_capture':True,'private_fields_covered':covered,'original_sha256':hashlib.sha256(raw).hexdigest(),'published_sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'manual_visual_review_required':True}

def capture(port,private,output,caption='',font_path=None):
    private.mkdir(parents=True,exist_ok=True);output.parent.mkdir(parents=True,exist_ok=True)
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}));base=f'http://127.0.0.1:{port}/'
    snap=json.load(opener.open(base+'snap?all=1',timeout=8));raw=opener.open(base+'g?raw=1',timeout=30).read()
    (private/(output.stem+'.widgets.private.json')).write_text(json.dumps(snap),'utf8');(private/(output.stem+'.original.private.png')).write_bytes(raw)
    return render_capture(raw,snap,output,caption,font_path)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--port',type=int,required=True);p.add_argument('--private-directory',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--caption',default='');p.add_argument('--font',type=pathlib.Path);a=p.parse_args();print(json.dumps(capture(a.port,a.private_directory,a.output,a.caption,a.font)))
