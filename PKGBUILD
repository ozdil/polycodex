# Maintainer: Ozan Ozdil <ozdil@example.com>
pkgname=polycodex
pkgver=0.1.0
pkgrel=1
pkgdesc="1-1 Mizanpaj ve gorselleri koruyan, 5000 sayfa kapasiteli evrensel acik kaynakli PDF cevirici"
arch=('x86_64' 'aarch64')
url="https://github.com/ozdil/polycodex"
license=('MIT' 'Apache-2.0')
depends=('gcc-libs' 'glibc')
optdepends=(
    'quickshell: Modern ve hafif QML masaustu kullanici arayuzu'
    'zenity: Grafiksel dosya ve klasor secim pencereleri'
    'ollama: Yerel acik kaynakli yapay zeka ceviri modelleri'
    'libretranslate: Yerel veya uzak REST ceviri sunucusu destegi'
    'poppler: PDF piksel render ve gorsel kontrol araclari (pdftoppm, pdfimages)'
)
makedepends=('cargo')
source=("$pkgname-$pkgver.tar.gz::$url/archive/v$pkgver.tar.gz")
sha256sums=('9b40c2b7454bd28b5496eb5fbc9579f2bd38bff0b8c986af0893b53799b1ae01')

prepare() {
    cd "$pkgname-$pkgver"
    cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release --bin polycodex
}

check() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo test --frozen --all-targets
}

package() {
    cd "$pkgname-$pkgver"

    # 1. Ikili Dosyalar (Executables)
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
    install -Dm755 polycodex-gui "$pkgdir/usr/bin/polycodex-gui"

    # 2. QML Masaustu Arayuz Bilesenleri
    install -Dm644 qml/shell.qml "$pkgdir/usr/share/$pkgname/qml/shell.qml"
    install -Dm644 qml/MainWindow.qml "$pkgdir/usr/share/$pkgname/qml/MainWindow.qml"
    install -Dm644 qml/Theme.qml "$pkgdir/usr/share/$pkgname/qml/Theme.qml"

    # 3. XDG Masaustu Entegrasyonu ve Ikon
    install -Dm644 polycodex.desktop "$pkgdir/usr/share/applications/polycodex.desktop"
    install -Dm644 polycodex.svg "$pkgdir/usr/share/icons/hicolor/scalable/apps/polycodex.svg"

    # 4. Lisans Metinleri (Arch MIT standardi)
    install -Dm644 LICENSE-MIT "$pkgdir/usr/share/licenses/$pkgname/LICENSE-MIT"
    install -Dm644 LICENSE-APACHE "$pkgdir/usr/share/licenses/$pkgname/LICENSE-APACHE"

    # 5. Dokumantasyon
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
    install -Dm644 CONTRIBUTING.md "$pkgdir/usr/share/doc/$pkgname/CONTRIBUTING.md"
}
