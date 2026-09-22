#!/usr/bin/env bash
set -euo pipefail

# PolyCodex AUR Tek Tikla Yayinlama Betigi

AUR_HOST="aur@aur.archlinux.org"

echo "==> AUR SSH yetkilendirmesi denetleniyor..."
AUTH_OUTPUT=$(ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -T "$AUR_HOST" 2>&1 || true)

if echo "$AUTH_OUTPUT" | grep -q "Interactive shell is disabled"; then
    echo "==> SSH yetkilendirmesi basarili!"
else
    echo "UYARI: AUR SSH erisimi henuz tanimli degil."
    echo "Lutfen https://aur.archlinux.org adresine giris yapip 'My Account' sekmesindeki"
    echo "'SSH Public Key' alanina asagidaki anahtari ekleyiniz:"
    echo "--------------------------------------------------------------------------------"
    cat ~/.ssh/id_ed25519.pub
    echo "--------------------------------------------------------------------------------"
    echo "Anahtari kaydettikten sonra bu betigi tekrar calistiriniz: ./publish-to-aur.sh"
    exit 1
fi

AUR_DIR="/tmp/polycodex-aur-release"
rm -rf "$AUR_DIR"

echo "==> AUR deposu klonlaniyor..."
git clone "ssh://${AUR_HOST}/polycodex.git" "$AUR_DIR"

echo "==> Paket dosyalari senkronize ediliyor..."
cp PKGBUILD "$AUR_DIR/"
cp .SRCINFO "$AUR_DIR/"

cd "$AUR_DIR"
git add PKGBUILD .SRCINFO

if git diff --staged --quiet; then
    echo "==> AUR deposu zaten guncel. Herhangi bir degisiklik yok."
else
    git commit -m "Publish initial release v0.1.0"
    echo "==> AUR sunucularina gonderiliyor (git push origin master)..."
    git push origin master
    echo "================================================================================"
    echo "TEBRIKLER! PolyCodex AUR uzerinde basariyla yayinlandi."
    echo "Paket Adresi: https://aur.archlinux.org/packages/polycodex"
    echo "Kurulum Komutu: yay -S polycodex (veya paru -S polycodex)"
    echo "================================================================================"
fi
