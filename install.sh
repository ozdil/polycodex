#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# PolyCodex Evrensel Linux Tek Komutla Kurulum Betigi
# Desteklenen Dagitimlar: Arch Linux, Ubuntu, Debian, Fedora, openSUSE, Manjaro vb.
# ==============================================================================

REPO="ozdil/polycodex"
INSTALL_PREFIX="/usr/local"
BIN_DIR="${INSTALL_PREFIX}/bin"
SHARE_DIR="${INSTALL_PREFIX}/share"
DATA_DIR="${SHARE_DIR}/polycodex"
DESKTOP_DIR="${SHARE_DIR}/applications"
ICON_DIR="${SHARE_DIR}/icons/hicolor/scalable/apps"

echo "===================================================================="
echo "  PolyCodex - Evrensel PDF Cevirici Kurulumu"
echo "===================================================================="

# 1. Root / Sudo Yetkisi Kontrolu
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    if command -v sudo &> /dev/null; then
        SUDO="sudo"
    else
        echo "HATA: Kurulum icin root yetkisi gereklidir. Lutfen 'sudo' ile calistiriniz." >&2
        exit 1
    fi
fi

# 2. Mimari Kontrolu (x86_64)
ARCH=$(uname -m)
if [ "$ARCH" != "x86_64" ]; then
    echo "HATA: Su an sadece x86_64 mimarisi desteklenmektedir. Algilanan: $ARCH" >&2
    exit 1
fi

# 3. En Son Surum Bilgisini Al
echo "==> En guncel surum GitHub uzerinden sorgulaniyor..."
LATEST_TAG=$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_TAG" ]; then
    LATEST_TAG="v0.1.0"
fi
echo "==> Tespit edilen surum: $LATEST_TAG"

# 4. Gecici Dizin Olustur
TMP_DIR=$(mktemp -d /tmp/polycodex-install-XXXXXX)
trap 'rm -rf "$TMP_DIR"' EXIT

TAR_URL="https://github.com/${REPO}/releases/download/${LATEST_TAG}/polycodex-${LATEST_TAG}-x86_64-linux.tar.gz"
echo "==> Binary indiriliyor: $TAR_URL"
curl -sSL "$TAR_URL" -o "${TMP_DIR}/polycodex.tar.gz"

echo "==> Kaynak varliklar indiriliyor..."
curl -sSL "https://raw.githubusercontent.com/${REPO}/main/polycodex-gui" -o "${TMP_DIR}/polycodex-gui"
curl -sSL "https://raw.githubusercontent.com/${REPO}/main/polycodex.desktop" -o "${TMP_DIR}/polycodex.desktop"
curl -sSL "https://raw.githubusercontent.com/${REPO}/main/polycodex.svg" -o "${TMP_DIR}/polycodex.svg"

mkdir -p "${TMP_DIR}/qml"
curl -sSL "https://raw.githubusercontent.com/${REPO}/main/qml/shell.qml" -o "${TMP_DIR}/qml/shell.qml"
curl -sSL "https://raw.githubusercontent.com/${REPO}/main/qml/MainWindow.qml" -o "${TMP_DIR}/qml/MainWindow.qml"
curl -sSL "https://raw.githubusercontent.com/${REPO}/main/qml/Theme.qml" -o "${TMP_DIR}/qml/Theme.qml"

tar -xzf "${TMP_DIR}/polycodex.tar.gz" -C "${TMP_DIR}"

# 5. Dizinleri Olustur ve Dosyalari Tasi
echo "==> Dosyalar sisteme yerlestiriliyor..."
$SUDO mkdir -p "$BIN_DIR" "$DATA_DIR/qml" "$DESKTOP_DIR" "$ICON_DIR"

$SUDO install -m 755 "${TMP_DIR}/polycodex" "${BIN_DIR}/polycodex"
$SUDO install -m 755 "${TMP_DIR}/polycodex-gui" "${BIN_DIR}/polycodex-gui"

$SUDO install -m 644 "${TMP_DIR}/qml/shell.qml" "${DATA_DIR}/qml/shell.qml"
$SUDO install -m 644 "${TMP_DIR}/qml/MainWindow.qml" "${DATA_DIR}/qml/MainWindow.qml"
$SUDO install -m 644 "${TMP_DIR}/qml/Theme.qml" "${DATA_DIR}/qml/Theme.qml"

$SUDO install -m 644 "${TMP_DIR}/polycodex.desktop" "${DESKTOP_DIR}/polycodex.desktop"
$SUDO install -m 644 "${TMP_DIR}/polycodex.svg" "${ICON_DIR}/polycodex.svg"

# 6. Masaustu Veritabanini Guncelle
if command -v update-desktop-database &> /dev/null; then
    $SUDO update-desktop-database "$DESKTOP_DIR" &> /dev/null || true
fi
if command -v gtk-update-icon-cache &> /dev/null; then
    $SUDO gtk-update-icon-cache -q -t -f "${SHARE_DIR}/icons/hicolor" &> /dev/null || true
fi

echo "===================================================================="
echo "KURULUM BASARIYLA TAMAMLANDI!"
echo "  - CLI Komutu : polycodex --help"
echo "  - GUI Komutu : polycodex-gui"
echo "  - Uygulama Menusu: 'PolyCodex' olarak arayabilirsiniz."
echo "===================================================================="
