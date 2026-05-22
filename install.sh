#!/usr/bin/env bash
# install.sh — install sem and mex from the latest GitHub release
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/rolze/mex/main/install.sh | bash
#
# What it does:
#   1. Verifies Linux x86_64 and that curl is available
#   2. Detects whether libvips is present and picks the right sem variant
#   3. Fetches the latest release tag from the GitHub API
#   4. Downloads sem and mex to a temp dir, makes them executable
#   5. Installs to /usr/local/bin (system-wide) or ~/.local/bin (user)
#   6. Checks runtime deps (sem libraries, mpv for video) and warns if any are missing

set -euo pipefail

REPO="rolze/mex"
API_URL="https://api.github.com/repos/${REPO}/releases/latest"
BASE_URL="https://github.com/${REPO}/releases/download"

# ── helpers ───────────────────────────────────────────────────────────────────

green()  { printf '\033[0;32m✓ %s\033[0m\n' "$*"; }
yellow() { printf '\033[0;33m⚠ %s\033[0m\n' "$*"; }
red()    { printf '\033[0;31m✗ %s\033[0m\n' "$*"; }
info()   { printf '  %s\n' "$*"; }

die() {
    red "$*"
    exit 1
}

# ── pre-flight checks ─────────────────────────────────────────────────────────

if [[ "$(uname -s)" != "Linux" ]]; then
    die "This installer only supports Linux. See INSTALL.md for other platforms."
fi

if [[ "$(uname -m)" != "x86_64" ]]; then
    die "Only x86_64 is supported. Got: $(uname -m). Build from source — see INSTALL.md."
fi

if ! command -v curl &>/dev/null; then
    die "curl is required but not found. Install it and retry."
fi

# ── detect libvips ────────────────────────────────────────────────────────────

SEM_VARIANT="sem-linux-x86_64"
SEM_LABEL="sem (image crate backend)"

if ldconfig -p 2>/dev/null | grep -q 'libvips'; then
    SEM_VARIANT="sem-linux-x86_64-vips"
    SEM_LABEL="sem (vips backend — preferred)"
fi

# ── fetch latest release tag ──────────────────────────────────────────────────

info "Fetching latest release info…"
TAG=$(curl -fsSL "${API_URL}" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
if [[ -z "$TAG" ]]; then
    die "Could not determine the latest release tag. Check your network connection."
fi
info "Latest release: ${TAG}"

# ── determine install directory ───────────────────────────────────────────────

INSTALL_DIR=""
if [[ -w "/usr/local/bin" ]] || sudo -n true 2>/dev/null; then
    INSTALL_DIR="/usr/local/bin"
    USE_SUDO=true
else
    INSTALL_DIR="${HOME}/.local/bin"
    USE_SUDO=false
    mkdir -p "${INSTALL_DIR}"
fi

# ── download ──────────────────────────────────────────────────────────────────

TMPDIR=$(mktemp -d)
trap 'rm -rf "${TMPDIR}"' EXIT

MEX_URL="${BASE_URL}/${TAG}/mex-linux-x86_64"
SEM_URL="${BASE_URL}/${TAG}/${SEM_VARIANT}"

info "Downloading mex…"
curl -fsSL --progress-bar -o "${TMPDIR}/mex" "${MEX_URL}" || \
    die "Download failed: ${MEX_URL}"

info "Downloading ${SEM_LABEL}…"
curl -fsSL --progress-bar -o "${TMPDIR}/sem" "${SEM_URL}" || \
    die "Download failed: ${SEM_URL}"

chmod +x "${TMPDIR}/mex" "${TMPDIR}/sem"

# ── install ───────────────────────────────────────────────────────────────────

if [[ "$USE_SUDO" == "true" && ! -w "/usr/local/bin" ]]; then
    sudo install -m 755 "${TMPDIR}/mex" "${INSTALL_DIR}/mex"
    sudo install -m 755 "${TMPDIR}/sem" "${INSTALL_DIR}/sem"
else
    install -m 755 "${TMPDIR}/mex" "${INSTALL_DIR}/mex"
    install -m 755 "${TMPDIR}/sem" "${INSTALL_DIR}/sem"
fi

green "mex installed to ${INSTALL_DIR}/mex"
green "${SEM_LABEL} installed to ${INSTALL_DIR}/sem"

# ── PATH hint ─────────────────────────────────────────────────────────────────

if [[ "$INSTALL_DIR" == "${HOME}/.local/bin" ]]; then
    if ! echo ":${PATH}:" | grep -q ":${HOME}/.local/bin:"; then
        yellow "~/.local/bin is not on your PATH."
        info "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
        info "  export PATH=\"\$HOME/.local/bin:\$PATH\""
    fi
fi

# ── runtime dependency checks ─────────────────────────────────────────────────

MISSING_DEPS=()

# ldconfig lives in /sbin or /usr/sbin which may not be on PATH in a
# non-interactive curl|bash session — resolve it explicitly.
_LDCONFIG=$(command -v ldconfig 2>/dev/null \
    || { for d in /sbin /usr/sbin /usr/local/sbin; do [[ -x "$d/ldconfig" ]] && echo "$d/ldconfig" && break; done; })

_has_lib() {
    local soname="$1" pkg="${2:-}"
    # Primary: scan the shared-library cache
    if [[ -n "$_LDCONFIG" ]] && "$_LDCONFIG" -p 2>/dev/null | grep -q "${soname}"; then
        return 0
    fi
    # Fallback: dpkg-query (Debian / Ubuntu)
    if [[ -n "$pkg" ]] && \
       dpkg-query -W -f='${Status}' "${pkg}" 2>/dev/null | grep -q 'install ok installed'; then
        return 0
    fi
    return 1
}

if ! _has_lib 'libgtk-4' 'libgtk-4-1'; then
    MISSING_DEPS+=("libgtk-4-1")
fi
if ! _has_lib 'libadwaita-1' 'libadwaita-1-0'; then
    MISSING_DEPS+=("libadwaita-1-0")
fi
if [[ "$SEM_VARIANT" == *"-vips" ]]; then
    if ! _has_lib 'libvips' 'libvips42' && ! _has_lib 'libvips' 'libvips42t64'; then
        # Suggest the right package name for the running Ubuntu/Debian version
        if grep -q "24\." /etc/os-release 2>/dev/null; then
            MISSING_DEPS+=("libvips42t64")
        else
            MISSING_DEPS+=("libvips42")
        fi
    fi
fi

if [[ ${#MISSING_DEPS[@]} -gt 0 ]]; then
    echo ""
    yellow "Missing runtime libraries detected — sem may not start."
    info "Install them with:"
    info "  sudo apt install ${MISSING_DEPS[*]}"
fi

# mpv is preferred for video playback — enables IPC controls (pause/resume,
# next/prev, media keys, live status). mex falls back to the OS default
# player if mpv is absent, but full integration requires mpv.
if ! command -v mpv &>/dev/null; then
    echo ""
    yellow "mpv not found — video will open in the OS default player (reduced integration)."
    info "Install mpv for full IPC controls (pause, next/prev, media keys):"
    info "  sudo apt install mpv"
fi

# ── done ──────────────────────────────────────────────────────────────────────

echo ""
green "All done! Run 'mex' to get started."
