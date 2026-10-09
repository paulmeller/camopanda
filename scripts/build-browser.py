#!/usr/bin/env python3
"""Build a pinned patched browser; verify Zig/V8 downloads; export its source."""
import argparse, gzip, hashlib, io, json, os, platform, shutil, subprocess, tarfile, urllib.request
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument('--work', type=Path, default=ROOT / '.runtime/browser-build')
p.add_argument('--output', type=Path, default=ROOT / 'dist')
p.add_argument('--test', action='store_true')
p.add_argument('--test-baseline', action='store_true', help='Compare the unpatched upstream suite before applying our patch (fresh work directory required)')
a = p.parse_args(); a.work = a.work.resolve(); a.output = a.output.resolve()
pins = json.loads((ROOT / 'browser/pins.json').read_text())
arch = {'arm64': 'aarch64', 'aarch64': 'aarch64', 'x86_64': 'x86_64'}.get(platform.machine())
osname = {'Darwin': 'macos', 'Linux': 'linux'}.get(platform.system())
key = f'{arch}-{osname}'
if key not in pins['zig_sha256']: raise SystemExit('supported: macOS arm64, Linux amd64/arm64')
a.work.mkdir(parents=True, exist_ok=True); a.output.mkdir(parents=True, exist_ok=True)
def run(*args, cwd=None, env=None): subprocess.run(args, cwd=cwd, env=env, check=True)
def download(url, path, digest):
    if not path.exists():
        tmp = path.with_suffix('.download'); urllib.request.urlretrieve(url, tmp); tmp.replace(path)
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise SystemExit(f'checksum mismatch: {path}')
zigdir = a.work / f'zig-{key}-{pins["zig"]}'
archive = a.work / 'zig.tar.xz'
download(f'https://ziglang.org/download/{pins["zig"]}/zig-{key}-{pins["zig"]}.tar.xz', archive, pins['zig_sha256'][key])
if not zigdir.exists():
    with tarfile.open(archive) as t:
        for member in t.getmembers():
            if Path(member.name).is_absolute() or '..' in Path(member.name).parts:
                raise SystemExit('invalid toolchain archive path')
        t.extractall(a.work)  # Official archive verified against the pinned SHA-256 above.
zig = str(zigdir / 'zig')
source = a.work / 'source'
if not source.exists():
    run('git', 'init', str(source)); run('git', '-C', str(source), 'remote', 'add', 'origin', pins['repository'])
    run('git', '-C', str(source), 'fetch', '--depth', '1', 'origin', pins['revision'])
    run('git', '-C', str(source), 'checkout', '--detach', 'FETCH_HEAD')
actual = subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip()
if actual != pins['revision']: raise SystemExit('browser work directory has wrong source revision')
patch = ROOT / 'browser/lightpanda.patch'
# Reuse only an exact patched source tree; refuse unknown local edits.
expected = patch.read_bytes()
current = subprocess.check_output(['git', '-C', str(source), 'diff', '--binary'])
if a.test_baseline and current: raise SystemExit('baseline comparison requires an unpatched source tree')
if current and current != expected: raise SystemExit('browser work directory contains a different patch; use a fresh --work directory')
v8file = f'libc_v8_{pins["v8_version"]}_{osname}_{arch}.a'
v8 = source / '.lp-cache/prebuilt-v8' / pins['v8_tag'] / v8file
v8.parent.mkdir(parents=True, exist_ok=True)
download(f'https://github.com/lightpanda-io/zig-v8-fork/releases/download/{pins["v8_tag"]}/{v8file}', v8, pins['v8_sha256'][key])
env = dict(os.environ); env.pop('CAMOPANDA_TEST_HEADERS', None); env['TEST_JOBS'] = '1'
target = [f'-Dtarget={arch}-linux-gnu.2.38'] if osname == 'linux' else []
if a.test_baseline:
    print('=== Unpatched upstream comparison (diagnostic; patched suite remains mandatory) ===', flush=True)
    baseline = subprocess.run(['make', 'test', f'ZIG={zig}', 'ZIGFLAGS=-j2 -Ddev_fast=false ' + ' '.join(target)], cwd=source, env=env)
    print(f'Unpatched upstream test exit code: {baseline.returncode}', flush=True)
if not current:
    run('git', 'apply', '--check', str(patch), cwd=source)
    run('git', 'apply', str(patch), cwd=source)
if a.test: run('make', 'test', f'ZIG={zig}', 'ZIGFLAGS=-j2 -Ddev_fast=false ' + ' '.join(target), cwd=source, env=env)
run(zig, 'build', '-Doptimize=fast', '-j2', *target, cwd=source, env=env)
shutil.copy2(source / 'zig-out/bin/lightpanda', a.output / 'lightpanda')
shutil.copy2(source / 'zig-out/bin/lightpanda', a.output / 'camopanda')
licenses = a.output / 'licenses/lightpanda'; licenses.mkdir(parents=True, exist_ok=True)
for name in ['LICENSE', 'LICENSING.md']: shutil.copy2(source / name, licenses / name)
shutil.copy2(ROOT / 'browser/pins.json', a.output / 'browser-pins.json')
# Fixed archive metadata makes the corresponding patched-source artifact stable.
files = subprocess.check_output(['git', 'ls-files', '-z'], cwd=source).split(b'\0')
with open(a.output / 'lightpanda-source.tar.gz', 'wb') as out:
    with gzip.GzipFile(fileobj=out, mode='wb', mtime=0) as gz:
        with tarfile.open(fileobj=gz, mode='w') as tar:
            for raw in sorted(f for f in files if f):
                name = raw.decode(); path = source / name
                info = tar.gettarinfo(str(path), arcname='lightpanda/' + name)
                info.uid = info.gid = info.mtime = 0; info.uname = info.gname = ''
                if path.is_file() and not path.is_symlink():
                    with open(path, 'rb') as f: tar.addfile(info, f)
                else: tar.addfile(info)
print(f'Built patched Lightpanda {pins["revision"]} for {key}; exported corresponding source.')
