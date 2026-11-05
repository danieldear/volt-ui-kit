#!/usr/bin/env python3
"""Exercise an already-installed generated starter on an explicit Android device.
No install, uninstall, permission grant, network request or personal-file action.
"""
import argparse
from pathlib import Path
import re
import subprocess
import time
import xml.etree.ElementTree as ET


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--serial',required=True)
    p.add_argument('--package',required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args()
    if not re.fullmatch(r'[a-zA-Z][\w]*(?:\.[a-zA-Z][\w]*)+',a.package):p.error('invalid package ID')
    a.output.mkdir(parents=True,exist_ok=True)
    def adb(*args):return subprocess.check_output(['adb','-s',a.serial,*args])
    remote='/sdcard/volt-starter-smoke.xml'
    def snapshot():
        adb('shell','uiautomator','dump',remote)
        raw=adb('exec-out','cat',remote);(a.output/'last-window.xml').write_bytes(raw)
        nodes=list(ET.fromstring(raw).iter('node'))
        assert not any('Volt mobile host' in n.get('text','') for n in nodes),'native host error visible'
        return nodes
    def node(key,by='content-desc'):
        found=[n for n in snapshot() if n.get(by)==key]
        assert found,f'missing {by}={key}';return found[0]
    def tap(n):
        x,y,r,b=map(int,re.findall(r'\d+',n.get('bounds')))
        adb('shell','input','tap',str((x+r)//2),str((y+b)//2));time.sleep(.2)
    def shot(name):(a.output/name).write_bytes(adb('exec-out','screencap','-p'))
    def proof():
        text=next(n.get('content-desc') for n in snapshot() if n.get('content-desc','').startswith('Volt renderer:'))
        return text,int(re.search(r'frames=(\d+)',text).group(1))
    try:
        adb('shell','am','start','-n',a.package+'/dev.volt.host.VoltActivity')
        assert node('increment').get('text')=='Count: 0','start from a freshly launched starter'
        text,frames=proof();assert frames>0;print(text,flush=True);shot('01-home.png')
        tap(node('increment'));assert node('increment').get('text')=='Count: 1'
        tap(node('navigation'));tap(node('name'))
        edit=node('android.widget.EditText','class');assert edit.get('text')=='Developer'
        adb('shell','input','keyevent','KEYCODE_MOVE_END')
        adb('shell','input','keyevent',*(['KEYCODE_DEL']*len('Developer')))
        adb('shell','input','text','Rust%sAndroid');assert node('android.widget.EditText','class').get('text')=='Rust Android'
        shot('02-native-editor.png');adb('shell','input','keyevent','KEYCODE_ENTER');time.sleep(.4)
        tap(node('save'));assert node('greeting').get('text')=='Hello, Rust Android';shot('03-saved.png')
        tap(node('navigation'));adb('shell','input','keyevent','KEYCODE_BACK');time.sleep(.3)
        assert node('greeting').get('text')=='Hello, Rust Android'
        # Resume preserves the Rust state, rather than manufacturing a fresh host.
        adb('shell','input','keyevent','KEYCODE_HOME');time.sleep(.3)
        adb('shell','am','start','-n',a.package+'/dev.volt.host.VoltActivity');time.sleep(.8)
        assert node('increment').get('text')=='Count: 1'
        assert node('greeting').get('text')=='Hello, Rust Android'
        last=False
        for _ in range(14):
            nodes=snapshot()
            if any(n.get('content-desc')=='item-30' for n in nodes):last=True;break
            canvas=next(n for n in nodes if n.get('content-desc','').startswith('Volt renderer:'))
            x,y,r,b=map(int,re.findall(r'\d+',canvas.get('bounds')))
            adb('shell','input','swipe',str((x+r)//2),str(b-200),str((x+r)//2),str(y+300),'350')
        assert last,'last row not reachable';shot('04-scrolled.png')
        _,before=proof();time.sleep(2);_,after=proof();assert before==after,'idle rendering loop detected'
        print('PASS: rendering, Rust callback, navigation, native editing/Save, platform back, resume, last-row scrolling and idle redraw stability',flush=True)
    finally:
        adb('shell','rm','-f',remote)

if __name__=='__main__':main()
