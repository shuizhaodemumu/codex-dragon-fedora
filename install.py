#!/usr/bin/env python3
"""Install a separate launcher; never replace the official app or its entry."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil

parser = argparse.ArgumentParser()
parser.add_argument('--uninstall', action='store_true')
args = parser.parse_args()
source = Path(__file__).resolve().parent
home = Path.home()
data = Path(os.environ.get('XDG_DATA_HOME', home / '.local/share'))
target = data / 'codex-dragon'
desktop = data / 'applications/codex-dragon.desktop'
binary = home / '.local/bin/codex-dragon'
manifest = target / 'install-manifest.json'
def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

if args.uninstall:
    if not manifest.exists():
        raise SystemExit('No tracked installation found.')
    state = json.loads(manifest.read_text())
    retained = []
    for name, expected in state['files'].items():
        path = Path(name)
        if path.is_symlink() or (path.exists() and digest(path) != expected):
            retained.append(name)
        elif path.is_file():
            path.unlink()
    if retained:
        raise SystemExit('Modified files retained; manifest preserved: ' + ', '.join(retained))
    manifest.unlink()
    for directory in (target / 'assets', target):
        try:
            directory.rmdir()
        except OSError:
            pass
    print('Codex Dragon removed; official Codex untouched.')
else:
    if target.exists() or desktop.exists() or binary.exists() or binary.is_symlink():
        raise SystemExit('Installation path already exists; no files changed.')
    video = source / 'assets/startup.mp4'
    exe = source / 'codex-dragon'
    if not video.is_file() or not exe.is_file():
        raise SystemExit('Missing compiled executable or animation.')
    (target / 'assets').mkdir(parents=True)
    binary.parent.mkdir(parents=True, exist_ok=True)
    desktop.parent.mkdir(parents=True, exist_ok=True)
    files = {}
    copies = [(exe, binary), (video, target / 'assets/startup.mp4'), (source / 'README.md', target / 'README.md'), (source / 'install.py', target / 'install.py'), (source / 'UPSTREAM-NOTICE.md', target / 'UPSTREAM-NOTICE.md')]
    for name in ('dragon-wallpaper-ambient-2k-144fps.mp4', 'dragon-wallpaper-particles-1440p48.mp4'):
        if (source / 'assets' / name).is_file():
            copies.append((source / 'assets' / name, target / 'assets' / name))
    for src, dest in copies:
        shutil.copy2(src, dest)
        files[str(dest)] = digest(dest)
    binary.chmod(0o755)
    quoted = str(binary).replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$')
    desktop_text = '[Desktop Entry]\nType=Application\nName=Codex Dragon\nName[zh_CN]=Codex 龙娘启动动画\nComment=Rust startup animation and dynamic wallpaper launcher\nExec="' + quoted + '" -- %U\nIcon=chatgpt\nTerminal=false\nCategories=Development;\nStartupNotify=false\nActions=Preview;WallpaperOff;WallpaperOn;\n'
    for action, label, flag in (('Preview', '预览启动动画', '--preview'), ('WallpaperOff', '关闭本次动态壁纸', '--wallpaper-off'), ('WallpaperOn', '重新启用动态壁纸', '--wallpaper-on')):
        desktop_text += '\n[Desktop Action ' + action + ']\nName=' + label + '\nExec="' + quoted + '" ' + flag + '\n'
    desktop.write_text(desktop_text, encoding='utf-8')
    files[str(desktop)] = digest(desktop)
    manifest.write_text(json.dumps({'files': files}, indent=2), encoding='utf-8')
    print('Installed:', binary)
    print('Desktop entry:', desktop)
    print('Uninstall: python3 ' + str(target / 'install.py') + ' --uninstall')
