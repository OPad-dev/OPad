# Maintainer: GFerreiroS <info@gferreiro.com>
pkgname=opad
pkgver=1.0.0
pkgrel=1
pkgdesc="Low-latency ESP32-S3 keypad manager, telemetry HUD, and tray applet for OPad"
arch=('x86_64')
url="https://github.com/OPad-dev/OPad"
license=('MIT' 'LGPL-3.0-only' 'OFL-1.1')

# Runtime dependencies:
# - iced/wgpu needs Vulkan loader and client libraries (Wayland + X11)
# - ksni needs dbus
# - tosu wrapper needs nodejs
# Node version risk: tosu upstream specifies Node 24.x engine. If current arch
# nodejs introduces incompatibilities, depend on the Node 24 LTS package instead of nodejs (tosu needs >=24.14 <25).
depends=(
    'vulkan-loader'
    'wayland'
    'libx11'
    'libxcursor'
    'libxkbcommon'
    'libxi'
    'libxrandr'
    'dbus'
    'nodejs'
)
optdepends=(
    'vulkan-driver: Hardware Vulkan acceleration for iced/wgpu GUI renderer'
)
makedepends=(
    'cargo'
    'git'
    'pnpm'
    'nodejs'
)
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
    cd "$srcdir/$pkgname-$pkgver"
    make all
    make tosu TOSU_STANDALONE=0
}

check() {
    cd "$srcdir/$pkgname-$pkgver"
    make check
}

package() {
    cd "$srcdir/$pkgname-$pkgver"
    make DESTDIR="$pkgdir" PREFIX=/usr INSTALL_ORIGIN=aur install
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    install -Dm644 desktop/gui/assets/fonts/Montserrat-OFL.txt "$pkgdir/usr/share/licenses/$pkgname/Montserrat-OFL.txt"
    install -Dm644 licenses/tosu/LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE.tosu"
}
