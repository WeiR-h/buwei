"""Capture the actual native render and explicitly cover private UI fields.

Read-only loopback RPC. No input events or business actions. Keep original
captures and full widget snapshots in an ignored private directory. Published
images must also receive visual review; this is a narrow privacy aid, not OCR.
Requires Pillow only when preparing demonstration media, not when running BuWei.
"""
import argparse,hashlib,io,json,pathlib,re,urllib.request
from PIL import Image,ImageDraw,ImageFont
PRIVATE=re.compile(r'@[A-Za-z0-9]|![A-Za-z0-9]|\$[A-Za-z0-9_-]{8,}|[a-fA-F0-9]{32,}|[A-Z]:[\\/]Users[\\/]|(?:sk-|gh[pousr]_)[A-Za-z0-9_-]{20,}')

def capture(port,private,output,caption='',font_path=None):
    private.mkdir(parents=True,exist_ok=True);output.parent.mkdir(parents=True,exist_ok=True)
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}));base=f'http://127.0.0.1:{port}/'
    snap=json.load(opener.open(base+'snap?all=1',timeout=8));raw=opener.open(base+'g?raw=1',timeout=8).read()
    (private/(output.stem+'.widgets.private.json')).write_text(json.dumps(snap),'utf8');(private/(output.stem+'.original.private.png')).write_bytes(raw)
    picture=Image.open(io.BytesIO(raw)).convert('RGB');draw=ImageDraw.Draw(picture)
    # The host's configured window is 1400 x 900 logical units; RPC raster
    # size follows DPI. Coordinates below come from its actual widget snapshot.
    scale_x=picture.width/1400;scale_y=picture.height/900;covered=0
    for widget in snap['s']:
        text=widget.get('t','');rect=widget.get('r',[0,0,0,0])
        if not text or not PRIVATE.search(text) or rect[2]<=0 or rect[3]<=0:continue
        x,y,w,h=rect
        lines=text.splitlines()
        if widget.get('i')=='activity' and len(lines)==3:
            targets=[(x,y+n*h/3,w,h/3) for n,line in enumerate(lines) if PRIVATE.search(line)]
        else:targets=[rect]
        for rx,ry,rw,rh in targets:
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

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--port',type=int,required=True);p.add_argument('--private-directory',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--caption',default='');p.add_argument('--font',type=pathlib.Path);a=p.parse_args();print(json.dumps(capture(a.port,a.private_directory,a.output,a.caption,a.font)))
