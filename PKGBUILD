# Maintainer: Praveen <praveen@local>
pkgname=agym-git
pkgver=1.0.0
pkgrel=1
pkgdesc="Unified session, account, and quota manager for Antigravity CLI"
arch=('any')
url="https://github.com/Praveensenpai/agym"
license=('MIT')
depends=('bash' 'fzf' 'jq' 'libsecret' 'sqlite' 'curl')
makedepends=('git')
provides=('agym')
conflicts=('agym')
source=("git+https://github.com/Praveensenpai/agym.git")
sha256sums=('SKIP')

pkgver() {
  cd "$srcdir/${pkgname%-git}" 2>/dev/null || cd "$srcdir"
  git describe --long --tags 2>/dev/null | sed 's/\([^-]*-g\)/r\1/;s/-/./g' || echo "1.0.0"
}

package() {
  cd "$srcdir/${pkgname%-git}" 2>/dev/null || cd "$srcdir"
  install -Dm755 bin/agym "$pkgdir/usr/bin/agym"
}
