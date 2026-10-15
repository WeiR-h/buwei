"""Own-app operations documented by the pinned Makepad remote bridge."""
import json,pathlib,time,urllib.parse,urllib.request
class NativeUI:
    def __init__(self,port,version='0.2.12'):
        self.port=port;self.version=version
        self.opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def get(self,path,**values):
        url=f'http://127.0.0.1:{self.port}/'+path
        if values:url+='?'+urllib.parse.urlencode(values)
        return self.opener.open(url,timeout=65).read()
    def tree(self):return json.loads(self.get('snap',all=1))['s']
    def window(self,tree=None):
        tree=tree or self.tree();status=json.loads(self.get('s'))
        for window in status['w']:
            texts=[s.get('t','') for s in tree if s.get('w')==window['i'] and s.get('v',1)!=0 and s.get('ty')=='Label']
            if any('刚好有位' in t for t in texts) and any('v'+self.version in t for t in texts):return window
        raise RuntimeError('Visible BuWei surface unavailable; no accepted capture')
    def element(self,identifier):
        tree=self.tree();window=self.window(tree)
        matches=[s for s in tree if s.get('i')==identifier and s.get('w')==window['i'] and s.get('v',1)!=0 and s.get('r',[0,0,0,0])[2]>0]
        interactive=[s for s in matches if s['ty'] in ('Button','TextInput')]
        if interactive:matches=interactive
        if len(matches)!=1:raise RuntimeError(f'Current widget is unavailable or ambiguous: {identifier}')
        return matches[0],window
    def scroll(self,dy):
        tree=self.tree();window=self.window(tree)
        pages={'page_activity','page_discover','page_goals','page_history','page_article','page_help'}
        candidates=[s for s in tree if s.get('w')==window['i'] and s['i'] in pages and s.get('v',1)!=0 and s['r'][2]>0]
        if not candidates:raise RuntimeError('No observed scroll surface')
        area=max(candidates,key=lambda s:s['r'][2]*s['r'][3]);x,y,width,height=area['r']
        self.get('m',k='scroll',w=window['i'],x=x+width-4,y=y+height*.45,dy=dy,dx=0,precise=1,wait=1)
        time.sleep(.3)
    def click(self,identifier):
        for attempt in range(16):
            try:item,window=self.element(identifier)
            except RuntimeError:
                tree=self.tree();window=self.window(tree)
                if not any(s['i']==identifier and s.get('w')==window['i'] and s['ty'] in ('Button','TextInput') for s in tree):raise
                self.scroll(350);continue
            x,y,w,h=item['r']
            # Keep clicks inside the main work area. Read a new widget tree
            # after every scroll instead of reusing stale coordinates.
            minimum=35 if identifier.startswith('nav_') or identifier=='new_activity' else 90
            if y>=minimum and y+h<=window['sz'][1]-100:
                self.get('click',w=window['i'],x=x+w/2,y=y+h/2,wait=1);time.sleep(.3);return
            self.scroll(350 if y+h>window['sz'][1]-70 else -350)
        raise RuntimeError('Widget cannot be brought into the current viewport: '+identifier)
    def fill(self,identifier,value):
        self.click(identifier);window=self.window()
        self.get('k',w=window['i'],c='KeyA',ctrl=1,wait=1)
        self.get('t',w=window['i'],t=value,wait=1);time.sleep(.2)
        if self.element(identifier)[0].get('t')!=value:raise RuntimeError('Input did not retain the requested text: '+identifier)
    def capture(self,target):
        tree=self.tree();window=self.window(tree)
        raw=self.get('g',w=window['i'],raw=1)
        assert raw.startswith(b'\x89PNG\r\n\x1a\n')
        target=pathlib.Path(target);target.parent.mkdir(parents=True,exist_ok=True)
        target.write_bytes(raw)
        target.with_suffix('.widgets.private.json').write_text(json.dumps(tree,ensure_ascii=False),'utf8')
        return {'file':target.name,'window':window,'locally_collected_at_unix':time.time()}
