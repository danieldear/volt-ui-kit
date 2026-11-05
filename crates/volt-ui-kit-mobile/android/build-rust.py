#!/usr/bin/env python3
"""Build one Rust cdylib for the generated Android project (no shell/eval)."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('manifest',type=Path)
    p.add_argument('abi',choices=['arm64-v8a','x86_64'])
    p.add_argument('ndk',type=Path)
    p.add_argument('output',type=Path)
    a=p.parse_args()
    manifest=a.manifest.resolve()
    args=['cargo','metadata','--format-version','1','--manifest-path',str(manifest)]
    if manifest.with_name('Cargo.lock').exists(): args.append('--locked')
    metadata=json.loads(subprocess.check_output(args))
    root=next(x for x in metadata['packages'] if x['id']==metadata['resolve']['root'])
    libraries=[x['name'] for x in root['targets'] if 'cdylib' in x['crate_types']]
    if len(libraries)!=1: p.error('app must have exactly one cdylib target')
    target={'arm64-v8a':'aarch64-linux-android','x86_64':'x86_64-linux-android'}[a.abi]
    platform={'darwin':'darwin-x86_64','linux':'linux-x86_64','win32':'windows-x86_64'}.get(sys.platform)
    if platform is None: p.error('unsupported NDK build host')
    tools=a.ndk/'toolchains/llvm/prebuilt'/platform/'bin'
    linker=tools/(target+'30-clang'+('.cmd' if os.name=='nt' else ''))
    if not linker.is_file(): p.error('NDK LLVM compiler missing: '+str(linker))
    env=os.environ.copy()
    key=target.upper().replace('-','_')
    env['CARGO_TARGET_'+key+'_LINKER']=str(linker)
    # Android 16 KiB page-size compatibility; do not affect desktop or iOS.
    env['CARGO_TARGET_'+key+'_RUSTFLAGS']='-C link-arg=-Wl,-z,max-page-size=16384'
    subprocess.run(['cargo','build','--locked','--release','--manifest-path',str(manifest),'--target',target],env=env,check=True)
    out=a.output/a.abi;out.mkdir(parents=True,exist_ok=True)
    binary=Path(metadata['target_directory'])/target/'release'/('lib'+libraries[0]+'.so')
    shutil.copyfile(binary,out/'libvolt_app.so')

if __name__=='__main__':main()
