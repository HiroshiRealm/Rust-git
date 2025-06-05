#!/usr/bin/env bash
set -euo pipefail
chmod +x "$0"

# 测试 repack 功能
tmp=$(mktemp -d)
mkdir -p "$tmp/repo"
cp rust-git "$tmp/repo/"
cd "$tmp/repo"

./rust-git init
# 生成多个 loose 对象
echo "a" > file1
./rust-git add file1
./rust-git commit -m "Add file1"
echo "b" > file2
./rust-git add file2
./rust-git commit -m "Add file2"

# 统计 loose 对象数量
loose_before=$(find .git/objects -type f ! -path "./.git/objects/pack/*" | wc -l)
echo "loose_before=$loose_before"

# 执行 repack
./rust-git repack

# 验证生成 pack 和 idx 文件
pack_count=$(ls .git/objects/pack/*.pack | wc -l)
idx_count=$(ls .git/objects/pack/*.idx | wc -l)
if [ "$pack_count" -ne 1 ] || [ "$idx_count" -ne 1 ]; then
  echo "repack failed: pack_count=$pack_count, idx_count=$idx_count"
  exit 1
fi

# 验证 loose 对象已被删除
loose_after=$(find .git/objects -type f ! -path ".git/objects/pack/*" | wc -l)
if [ "$loose_after" -ne 0 ]; then
  echo "loose objects not cleaned: $loose_after"
  exit 1
fi

# 测试 gc 功能
# 生成一个新的对象并使其不可达
echo "c" > file3
./rust-git add file3
./rust-git commit -m "Add file3"
./rust-git branch temp_branch
echo "d" > file4
./rust-git add file4
./rust-git commit -m "Add file4"

# 切回主分支并删除临时分支
./rust-git checkout main || ./rust-git checkout master
./rust-git branch -D temp_branch

# 执行 gc
./rust-git gc

# 验证 unreachable 对象已被删除
loose_after_gc=$(find .git/objects -type f ! -path ".git/objects/pack/*" | wc -l)
if [ "$loose_after_gc" -ne 0 ]; then
  echo "gc failed: loose_after_gc=$loose_after_gc"
  exit 1
fi

echo "GC and repack tests passed"
exit 0