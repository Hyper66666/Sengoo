#!/usr/bin/env sh
set -eu

archive=""
version=""
target=""
base_url="https://github.com/Hyper66666/Sengoo/releases/download"
install_dir="$HOME/.sengoo"
add_to_path=0

usage() {
  echo "usage: scripts/install.sh --version VERSION [--target TARGET] [--base-url URL] [--install-dir DIR] [--add-to-path]" >&2
  echo "   or: scripts/install.sh ARCHIVE [INSTALL_DIR]" >&2
  echo "   or: scripts/install.sh --print-target" >&2
}

default_target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os" in
    Linux*)
      case "$arch" in
        x86_64|amd64) echo "x86_64-unknown-linux-gnu" ;;
        aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) echo "${arch}-unknown-linux-gnu" ;;
      esac
      ;;
    Darwin*)
      case "$arch" in
        arm64|aarch64) echo "aarch64-apple-darwin" ;;
        x86_64|amd64) echo "x86_64-apple-darwin" ;;
        *) echo "${arch}-apple-darwin" ;;
      esac
      ;;
    MINGW*|MSYS*|CYGWIN*) echo "x86_64-pc-windows-msvc" ;;
    *) echo "x86_64-unknown-linux-gnu" ;;
  esac
}

download_file() {
  url=$1
  destination=$2
  echo "Downloading $url"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$destination"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$destination"
  else
    echo "curl or wget is required for downloads" >&2
    exit 1
  fi
}

copy_or_download_file() {
  source=$1
  destination=$2
  if [ -f "$source" ]; then
    echo "Copying $source"
    cp "$source" "$destination"
  else
    download_file "$source" "$destination"
  fi
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      version=${2:?missing version}
      shift 2
      ;;
    --target)
      target=${2:?missing target}
      shift 2
      ;;
    --base-url)
      base_url=${2:?missing base url}
      shift 2
      ;;
    --install-dir)
      install_dir=${2:?missing install dir}
      shift 2
      ;;
    --add-to-path)
      add_to_path=1
      shift
      ;;
    --print-target)
      default_target
      exit 0
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      if [ -z "$archive" ]; then
        archive=$1
      elif [ "$install_dir" = "$HOME/.sengoo" ]; then
        install_dir=$1
      else
        usage
        exit 2
      fi
      shift
      ;;
  esac
done

if [ -n "$archive" ] && [ -n "$version" ]; then
  echo "provide only one of ARCHIVE or --version" >&2
  exit 2
fi
if [ -z "$archive" ] && [ -z "$version" ]; then
  usage
  exit 2
fi

case "$install_dir" in
  /*) ;;
  *) install_dir="$(pwd)/$install_dir" ;;
esac

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/sengoo-install.XXXXXX")
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

if [ -n "$version" ]; then
  if [ -z "$target" ]; then
    target=$(default_target)
  fi
  case "$target" in
    *windows*) extension="zip" ;;
    *) extension="tar.gz" ;;
  esac
  archive_name="sengoo-$version-$target.$extension"
  archive="$tmp_dir/$archive_name"
  if [ -d "$base_url" ]; then
    source="${base_url%/}/v$version/$archive_name"
    checksum_source="${source}.sha256"
  else
    release_base="${base_url%/}/v$version"
    source="$release_base/$archive_name"
    checksum_source="$source.sha256"
  fi
  copy_or_download_file "$source" "$archive"
  copy_or_download_file "$checksum_source" "$archive.sha256"
fi

checksum_file="$archive.sha256"
if [ ! -f "$checksum_file" ]; then
  echo "checksum file not found: $checksum_file" >&2
  exit 1
fi
expected_hash=$(awk '{print tolower($1)}' "$checksum_file")
if command -v sha256sum >/dev/null 2>&1; then
  actual_hash=$(sha256sum "$archive" | awk '{print tolower($1)}')
elif command -v shasum >/dev/null 2>&1; then
  actual_hash=$(shasum -a 256 "$archive" | awk '{print tolower($1)}')
else
  echo "sha256sum or shasum is required for checksum verification" >&2
  exit 1
fi
if [ "$expected_hash" != "$actual_hash" ]; then
  echo "checksum mismatch for $archive" >&2
  exit 1
fi

case "$archive" in
  *.tar.gz|*.tgz)
    tar -xzf "$archive" -C "$tmp_dir"
    ;;
  *.zip)
    unzip -q "$archive" -d "$tmp_dir"
    ;;
  *)
    echo "unsupported archive type: $archive" >&2
    exit 2
    ;;
esac

payload=$(find "$tmp_dir" -mindepth 1 -maxdepth 2 -name manifest.json -print -quit)
if [ -z "$payload" ]; then
  echo "archive does not contain a Sengoo manifest.json" >&2
  exit 1
fi
payload_dir=$(dirname "$payload")
payload_checksums="$payload_dir/payloads.sha256"
if [ ! -f "$payload_checksums" ]; then
  if [ "$version" = "0.1.0-rc.1" ]; then
    echo "warning: v0.1.0-rc.1 predates payloads.sha256; relying on the verified release archive SHA-256" >&2
  else
    echo "archive does not contain payloads.sha256" >&2
    exit 1
  fi
else
  while IFS= read -r checksum_line; do
    payload_path=${checksum_line#*  }
    case "$payload_path" in
      ""|/*|../*|*/../*|*/..)
        echo "invalid payload checksum path: $payload_path" >&2
        exit 1
        ;;
    esac
  done < "$payload_checksums"
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$payload_dir" && sha256sum -c payloads.sha256)
  elif command -v shasum >/dev/null 2>&1; then
    (cd "$payload_dir" && shasum -a 256 -c payloads.sha256)
  else
    echo "sha256sum or shasum is required for payload verification" >&2
    exit 1
  fi
  (cd "$payload_dir" && find . -type f ! -name manifest.json ! -name payloads.sha256 -print | sed 's#^\./##' | LC_ALL=C sort) > "$tmp_dir/actual-payloads.txt"
  sed 's/^[0-9a-fA-F]\{64\}  //' "$payload_checksums" | LC_ALL=C sort > "$tmp_dir/listed-payloads.txt"
  if ! cmp -s "$tmp_dir/actual-payloads.txt" "$tmp_dir/listed-payloads.txt"; then
    echo "archive payload set does not match payloads.sha256" >&2
    exit 1
  fi
fi

rm -rf "$install_dir"
mkdir -p "$install_dir"
cp -R "$payload_dir"/. "$install_dir"/

if [ -x "$install_dir/bin/sgc" ]; then
  "$install_dir/bin/sgc" --version
fi

if [ "$add_to_path" -eq 1 ]; then
  profile=${SHELL_PROFILE:-"$HOME/.profile"}
  mkdir -p "$(dirname "$profile")"
  marker="# Sengoo toolchain PATH"
  if [ ! -f "$profile" ] || ! grep -F "$install_dir/bin" "$profile" >/dev/null 2>&1; then
    {
      echo ""
      echo "$marker"
      echo "export PATH=\"$install_dir/bin:\$PATH\""
    } >> "$profile"
    echo "Added $install_dir/bin to $profile. Open a new shell to use it."
  fi
else
  echo "Add this directory to PATH: $install_dir/bin"
fi

echo "Installed Sengoo to $install_dir"
