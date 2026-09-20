# -*- coding: utf-8 -*-
"""清理 GitHub Actions 废弃缓存（需要仓库 admin 权限的 gh 登录）。

背景（docs-local/PATCHES.md "CI release 缓存投毒"）：release 旧 scope
`v1-rust-release*` 的 blob 含 research 合入前的 workspace-crate 产物
（陈旧 rmeta），scope 已改名为 release-2 / release-xwin-2，旧 blob 永不
再被读取，仅占 10 GB 配额，可安全删除。另有旧 lockfile hash 的 build
blob 一并过期。

用法：
    python scripts-local/purge-gh-caches.py          # 预览（默认 dry-run）
    python scripts-local/purge-gh-caches.py --apply  # 实际删除

gh 登录账号需对 daidaiJ/grok-build-proxy 有 admin 权限，否则 DELETE 返回
403 "Must have admin rights to Repository"（只读账号可列不可删）。
"""
import json
import subprocess
import sys

REPO = 'daidaiJ/grok-build-proxy'

# 按 key 前缀匹配删除：旧 release scope（含带毒 blob）+ 旧 lockfile hash 的 build
DELETE_PREFIXES = [
    'v1-rust-release-',
    'v1-rust-release-xwin-',
    'v1-rust-build-Windows_NT-x64-4566bc01-9c6f845c',
    'v1-rust-build-Linux-x64-54fa28ac-',
]
# 明确保留：xwin SDK 缓存、当前 lockfile hash 的 build blob
KEEP_PREFIXES = [
    'xwin-sdk-',
    'v1-rust-build-Windows_NT-x64-4566bc01-3ff2ea85',
]


def should_delete(key):
    if any(key.startswith(p) for p in KEEP_PREFIXES):
        return False
    return any(key.startswith(p) for p in DELETE_PREFIXES)


def main():
    apply = '--apply' in sys.argv
    r = subprocess.run(['gh', 'api', '--paginate', f'/repos/{REPO}/actions/caches'],
                       capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f'list failed: {r.stderr[:300]}')
    caches = json.loads(r.stdout).get('actions_caches', [])
    total_freed = 0
    for c in caches:
        key, size = c['key'], c['size_in_bytes']
        if not should_delete(key):
            print(f"KEPT      {key}  {size / 2**20:.0f} MiB")
            continue
        if not apply:
            print(f"WOULD DEL {key}  {size / 2**20:.0f} MiB  (id={c['id']})")
            total_freed += size
            continue
        d = subprocess.run(['gh', 'api', '-X', 'DELETE',
                            f"/repos/{REPO}/actions/caches/{c['id']}"],
                           capture_output=True, text=True)
        if d.returncode == 0:
            total_freed += size
            print(f"DELETED   {key}  {size / 2**20:.0f} MiB")
        else:
            print(f"FAILED    {key}: {d.stderr.strip()[:120]}")
    print(f'--- {"freed" if apply else "would free"} {total_freed / 2**30:.2f} GiB')
    if not apply:
        print('（预览模式，加 --apply 执行删除）')


if __name__ == '__main__':
    main()
