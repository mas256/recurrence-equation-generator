#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
lean_dir="$project_dir/lean"
toolchain="leanprover/lean4:v4.19.0"
export PATH="$HOME/.elan/bin:$PATH"

echo "Lean 4.19.0 と固定した mathlib を準備します（初回は通信が必要です）。"
if ! command -v elan >/dev/null 2>&1; then
  installer="$(mktemp)"
  trap 'rm -f "$installer"' EXIT
  curl --fail --location --silent --show-error \
    https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh -o "$installer"
  bash "$installer" -y --default-toolchain none
fi

if ! elan toolchain install "$toolchain"; then
  # Some corporate proxies permit GitHub releases but block releases.lean-lang.org.
  # Download the same official version without changing proxy configuration.
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64) platform=linux ;;
    Linux:aarch64|Linux:arm64) platform=linux_aarch64 ;;
    Darwin:x86_64) platform=darwin ;;
    Darwin:arm64) platform=darwin_aarch64 ;;
    *) echo "この環境では Lean 4.19.0 を手動でインストールしてください。" >&2; exit 1 ;;
  esac
  archive_dir="$(mktemp -d)"
  lean_install_dir="$HOME/.local/share/recurrence-lean"
  mkdir -p "$lean_install_dir"
  curl --fail --location --show-error \
    "https://github.com/leanprover/lean4/releases/download/v4.19.0/lean-4.19.0-$platform.tar.zst" \
    -o "$archive_dir/lean.tar.zst"
  tar --zstd -xf "$archive_dir/lean.tar.zst" -C "$lean_install_dir"
  elan toolchain link "$toolchain" "$lean_install_dir/lean-4.19.0-$platform"
  rm -rf -- "$archive_dir"
fi

cd "$lean_dir"
lake env lean --version | tee .prepared.version
grep -F 'version 4.19.0,' .prepared.version >/dev/null
if [[ "${RECURRENCE_BUILD_FROM_SOURCE:-0}" != 1 ]]; then
  if ! lake exe cache get Mathlib/Data/Rat/Cast/Order.lean Mathlib/Algebra/BigOperators/Group/Finset/Basic.lean Mathlib/Data/Nat/Factorial/Basic.lean Mathlib/Data/Nat/Choose/Basic.lean Mathlib/Tactic/FieldSimp.lean Mathlib/Tactic/Linarith.lean Mathlib/Tactic/Ring.lean Mathlib/Tactic/Positivity.lean Mathlib/Tactic/NormNum.lean; then
    echo "キャッシュを取得できないため、必要なモジュールをソースから構築します。"
  fi
fi
lake build Recurrence
lake build Mathlib.Data.Nat.Choose.Basic
lake env lean Audit.lean > .prepared.audit
if grep -E 'sorryAx|(^|[[:space:]])(sorry|admit|axiom)([[:space:]]|$)' \
    Recurrence/Theory.lean .prepared.audit; then
  echo "未完成の証明または未証明公理を検出しました。" >&2
  exit 1
fi

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1"
  else
    shasum -a 256 "$1"
  fi
}
prepared_inputs="$(mktemp)"
for input in lean-toolchain lakefile.toml lake-manifest.json Recurrence.lean \
    Recurrence/Theory.lean Audit.lean .lake/build/lib/lean/Recurrence/Theory.olean \
    .prepared.version .prepared.audit; do
  hash_file "$input" >> "$prepared_inputs"
done
mv "$prepared_inputs" .prepared-inputs
echo "準備が完了しました。Rust アプリから 1 問ずつ検証できます。"
