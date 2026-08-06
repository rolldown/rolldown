#!/usr/bin/env bash

set -euo pipefail

readonly runtime_apex_url='https://android.googlesource.com/platform/prebuilts/runtime/+/7b5a7c7117dbd3243344b8f9d9076ea983e1afb0/mainline/runtime/apex/com.android.runtime-arm64.apex?format=TEXT'
readonly runtime_apex_sha256='ec2500bcef83fc2856433d20fda8dd7adf14a7c430375c1c8441d2c4ba6acb0f'
readonly known_bad_binding_url='https://registry.npmjs.org/@rolldown/binding-android-arm64/-/binding-android-arm64-1.2.3.tgz'
readonly known_bad_binding_sha256='aa01416cbdb2df106fe4224e1434636c74d29842b0b191f44b27660b6de5df57'

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <android-arm64-binding>" >&2
  exit 2
fi

readonly binding_path="$1"
if [[ ! -f "$binding_path" ]]; then
  echo "binding does not exist: $binding_path" >&2
  exit 2
fi

readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work_dir="$(mktemp -d)"
readonly work_dir

cleanup() {
  if [[ -n "${work_dir:-}" && -d "$work_dir" ]]; then
    rm -rf -- "$work_dir"
  fi
}
trap cleanup EXIT

download() {
  local url="$1"
  local destination="$2"
  curl --fail --location --retry 3 --retry-all-errors --silent --show-error "$url" --output "$destination"
}

verify_sha256() {
  local expected="$1"
  local file="$2"
  printf '%s  %s\n' "$expected" "$file" | sha256sum --check --status
}

find_ndk() {
  local ndk_home="${ANDROID_NDK_LATEST_HOME:-${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}}"
  if [[ -n "$ndk_home" && -d "$ndk_home" ]]; then
    printf '%s\n' "$ndk_home"
    return
  fi

  local sdk_root="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}"
  if [[ -n "$sdk_root" && -d "$sdk_root/ndk" ]]; then
    find "$sdk_root/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1
  fi
}

run_on_cpu() {
  local cpu="$1"
  shift
  timeout --signal=KILL 30s qemu-aarch64 -cpu "$cpu" -L "$runtime_root" "$@"
}

readonly ndk_home="$(find_ndk)"
readonly android_clang="$ndk_home/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang"
if [[ ! -x "$android_clang" ]]; then
  echo "Android NDK compiler does not exist: $android_clang" >&2
  exit 2
fi

readonly runtime_root="$work_dir/root"
mkdir -p "$runtime_root/system/bin" "$runtime_root/system/lib64"

echo 'Downloading the pinned 7.6 MiB AOSP Runtime APEX'
download "$runtime_apex_url" "$work_dir/runtime.apex.base64"
base64 --decode < "$work_dir/runtime.apex.base64" > "$work_dir/runtime.apex"
verify_sha256 "$runtime_apex_sha256" "$work_dir/runtime.apex"
unzip -p "$work_dir/runtime.apex" apex_payload.img > "$work_dir/apex_payload.img"

debugfs -R 'cat /bin/linker64' "$work_dir/apex_payload.img" > "$runtime_root/system/bin/linker64"
debugfs -R 'cat /lib64/bionic/libc.so' "$work_dir/apex_payload.img" > "$runtime_root/system/lib64/libc.so"
debugfs -R 'cat /lib64/bionic/libdl.so' "$work_dir/apex_payload.img" > "$runtime_root/system/lib64/libdl.so"
debugfs -R 'cat /lib64/bionic/libm.so' "$work_dir/apex_payload.img" > "$runtime_root/system/lib64/libm.so"
chmod 755 "$runtime_root/system/bin/linker64"

"$android_clang" -O2 -Wall -Wextra -Werror "$script_dir/loader.c" -ldl -o "$runtime_root/loader"
"$android_clang" -nostdlib -static -Wl,--build-id=none -Wl,-e,_start "$script_dir/lse_probe.S" -o "$work_dir/lse-probe"

echo 'Checking that the selected QEMU CPU rejects Arm LSE instructions'
run_on_cpu max "$work_dir/lse-probe"
set +e
run_on_cpu cortex-a53 "$work_dir/lse-probe" > "$work_dir/lse-probe.log" 2>&1
probe_status=$?
set -e
if [[ $probe_status -eq 0 ]]; then
  echo 'Cortex-A53 unexpectedly executed an Arm LSE instruction' >&2
  exit 1
fi
if [[ $probe_status -eq 124 || $probe_status -eq 137 ]]; then
  echo 'Arm LSE probe timed out' >&2
  exit 1
fi

echo 'Checking the Bionic loader with an ordinary system library'
run_on_cpu cortex-a53 "$runtime_root/loader" /system/lib64/libm.so

echo 'Checking that the published v1.2.3 binding reproduces the original SIGILL'
download "$known_bad_binding_url" "$work_dir/known-bad-binding.tgz"
verify_sha256 "$known_bad_binding_sha256" "$work_dir/known-bad-binding.tgz"
mkdir -p "$work_dir/known-bad-binding"
tar -xzf "$work_dir/known-bad-binding.tgz" -C "$work_dir/known-bad-binding"
cp "$work_dir/known-bad-binding/package/rolldown-binding.android-arm64.node" "$runtime_root/known-bad.node"

set +e
run_on_cpu cortex-a53 "$runtime_root/loader" /known-bad.node > "$work_dir/known-bad.log" 2>&1
known_bad_status=$?
set -e
cat "$work_dir/known-bad.log"
if [[ $known_bad_status -eq 0 ]]; then
  echo 'The known-bad binding unexpectedly loaded on Cortex-A53' >&2
  exit 1
fi
if [[ $known_bad_status -eq 124 || $known_bad_status -eq 137 ]]; then
  echo 'The known-bad binding timed out instead of raising SIGILL' >&2
  exit 1
fi
if ! grep -Fq 'Fatal signal 4 (SIGILL)' "$work_dir/known-bad.log"; then
  echo 'The known-bad binding failed without the expected SIGILL' >&2
  exit 1
fi

echo 'Checking the binding built from the current commit'
cp "$binding_path" "$runtime_root/current.node"
run_on_cpu cortex-a53 "$runtime_root/loader" /current.node

echo 'Cortex-A53 regression check passed'
