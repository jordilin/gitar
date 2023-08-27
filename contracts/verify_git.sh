#!/bin/bash

contracts_dir=$(pwd)/contracts/git

tmpdir=$(mktemp -d /tmp/gitstatus.XXXXXXXXX)

function tear_down {
  cd - >/dev/null || exit
  rm -rf "$tmpdir"
}

function assert_eq {
  output=$1
  expected=$2
  contract_file=$3
  msg=$4
  if test "$output" != "$expected"; then
    echo
    echo
    echo "CONTRACT FAILURE: $msg. command: git status --short"
    echo "Expected: $expected"
    echo "Actual: $output"

    find_expectations "$contract_file"
  fi
}

function find_expectations {
  contract_file=$1
  cd - >/dev/null || exit
  echo
  echo "Expectations are broken: "
  git --no-pager grep -n "$contract_file"
  tear_down
  exit 1
}

## Init repository
cd "$tmpdir" || exit
git init . >/dev/null 2>&1
echo "# README" >"INSTALL.md"

echo -n "git status untracked files..."
expected=$(cat "$contracts_dir"/git_status_untracked_files.txt)
output=$(git status --short)
assert_eq "$output" "$expected" "git_status_untracked_files.txt" "untracked files"
echo "OK"

git add INSTALL.md
git commit -sm"Add readme" >/dev/null 2>&1

echo "Introduction" >>"INSTALL.md"
echo -n "git status modified files..."
expected=$(cat "$contracts_dir"/git_status_modified_files.txt)
output=$(git status --short)
assert_eq "$output" "$expected" "git_status_modified_files.txt" "modified files"
echo "OK"

touch "new-src.py"
echo -n "git status untracked & modified files..."
expected=$(cat "$contracts_dir"/git_status_untracked_and_modified_files.txt)
output=$(git status --short)
assert_eq "$output" "$expected" "git_status_untracked_and_modified_files.txt" "modified files"
echo "OK"

git add .
git commit -sm"Add readme and new file" >/dev/null 2>&1

echo -n "git status clean repository..."
expected=$(cat "$contracts_dir"/git_status_clean_repo.txt)
output=$(git status --short)
assert_eq "$output" "$expected" "git_status_clean_repo.txt" "clean repository"
echo "OK"

git branch -M main
echo -n "git current branch..."
expected=$(cat "$contracts_dir"/git_current_branch.txt)
output=$(git rev-parse --abbrev-ref HEAD)
assert_eq "$output" "$expected" "git_current_branch.txt" "current branch"
echo "OK"

echo -n "git push origin main..."
{
  git push origin main >/dev/null 2>&1 &&
    echo &&
    echo "CONTRACT FAILURE: remote does not exist" &&
    find_expectations "git_push_failure.txt"
} || echo "OK"

git remote add origin https://gitlab.com/jordilin/gitlapi.git
git fetch origin >/dev/null 2>&1

echo -n "git rebase origin/foo..."
{
  git rebase origin/foo >/dev/null 2>&1 &&
    echo &&
    echo "CONTRACT FAILURE: rebase wrong origin" &&
    find_expectations "git_rebase_wrong_origin.txt"
} || echo "OK"

tear_down
