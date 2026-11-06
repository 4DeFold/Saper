#!/usr/bin/env bash
# Сборка macOS .app и DMG строго для Apple Silicon (aarch64-apple-darwin).
# Запускать только на Mac с ARM: hdiutil/iconutil/codesign есть только в macOS.
set -euo pipefail

APP_NAME="Classic Minesweeper"
BIN_NAME="classic-minesweeper"
TARGET="aarch64-apple-darwin"
DMG_NAME="Classic-Minesweeper-Apple-Silicon.dmg"
ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

fail() {
  echo "Ошибка: $*" >&2
  exit 1
}

[[ "$(uname -s)" == "Darwin" ]] || fail "для упаковки .app/.dmg нужна macOS (hdiutil, iconutil, codesign)."
[[ "$(uname -m)" == "arm64" ]] || fail "сборка Apple Silicon требует ARM-машины, а uname -m = $(uname -m)."
command -v cargo >/dev/null 2>&1 || fail "не найден cargo"
command -v hdiutil >/dev/null 2>&1 || fail "не найден hdiutil"
command -v codesign >/dev/null 2>&1 || fail "не найден codesign"
command -v plutil >/dev/null 2>&1 || fail "не найден plutil"
command -v lipo >/dev/null 2>&1 || fail "не найден lipo"

export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
HOST="$(rustc -vV | sed -n 's/^host: //p')"
[[ "$HOST" == "$TARGET" ]] || fail "host rustc = '$HOST', ожидался '$TARGET'; ARM-архитектура не подтверждена."
if command -v rustup >/dev/null 2>&1; then
  rustup target add "$TARGET" >/dev/null 2>&1 || true
fi

VERSION="$(awk -F'"' '/^version = / { print $2; exit }' Cargo.toml)"
[[ -n "$VERSION" ]] || fail "не удалось определить версию из Cargo.toml"
echo "==> Архитектура: $(uname -m) / $TARGET, версия $VERSION"
echo "==> Сборка release-бинаря..."
cargo build --release --target "$TARGET" --bin "$BIN_NAME"

BUILD="build"
APP="$BUILD/$APP_NAME.app"
STAGE="$BUILD/dmg-stage"
rm -rf "$APP" "$STAGE"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$STAGE"
BIN_SRC="target/$TARGET/release/$BIN_NAME"
[[ -x "$BIN_SRC" ]] || fail "не найден собранный бинарь $BIN_SRC"
cp "$BIN_SRC" "$APP/Contents/MacOS/$BIN_NAME"
chmod +x "$APP/Contents/MacOS/$BIN_NAME"

# Атрибуция: шрифт и его лицензия едут вместе с приложением.
cp assets/DejaVuSans.ttf "$APP/Contents/Resources/DejaVuSans.ttf" || fail "нет assets/DejaVuSans.ttf"
cp assets/FONT-LICENSE.txt "$APP/Contents/Resources/FONT-LICENSE.txt" || fail "нет assets/FONT-LICENSE.txt"
cp LICENSE "$APP/Contents/Resources/LICENSE.txt" || fail "нет LICENSE"
cp README.md "$APP/Contents/Resources/README.md"
cp THIRD-PARTY-NOTICES.md "$APP/Contents/Resources/THIRD-PARTY-NOTICES.md" 2>/dev/null || true

# Иконка: из собственного assets/icon.png через системные sips + iconutil.
ICON_PLIST=""
if [[ -f assets/icon.png ]] && command -v sips >/dev/null 2>&1 && command -v iconutil >/dev/null 2>&1; then
  ICONSET="$BUILD/AppIcon.iconset"
  rm -rf "$ICONSET"
  mkdir -p "$ICONSET"
  for size in 16 32 128 256 512; do
    double=$((size * 2))
    sips -z "$size" "$size" assets/icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    sips -z "$double" "$double" assets/icon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
  ICON_PLIST="<key>CFBundleIconFile</key><string>AppIcon.icns</string>"
  echo "==> Иконка AppIcon.icns создана"
fi

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$APP_NAME</string>
  <key>CFBundleDisplayName</key><string>$APP_NAME</string>
  <key>CFBundleExecutable</key><string>$BIN_NAME</string>
  <key>CFBundleIdentifier</key><string>com.arena.classicminesweeper</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.games</string>
  <key>NSHighResolutionCapable</key><true/>
  $ICON_PLIST
</dict>
</plist>
PLIST

# На Apple Silicon бинарь без подписи не запускается. Это ad-hoc подпись,
# а НЕ Developer ID и не notarization.
codesign --force --deep --sign - "$APP" >/dev/null

echo "==> Проверка .app"
plutil -lint "$APP/Contents/Info.plist" >/dev/null || fail "Info.plist невалиден"
[[ -x "$APP/Contents/MacOS/$BIN_NAME" ]] || fail "исполняемый файл .app недоступен"
ARCHS="$(lipo -archs "$APP/Contents/MacOS/$BIN_NAME")"
echo "    lipo -archs: $ARCHS"
[[ "$ARCHS" == *arm64* ]] || fail "бинарь не arm64: $ARCHS"
[[ "$ARCHS" != *x86_64* ]] || fail "неожиданно найдена x86_64 в arm64-сборке: $ARCHS"
codesign -dv "$APP" 2>&1 | sed -n '1,6p' || true

echo "==> Упаковка DMG"
ln -s /Applications "$STAGE/Applications"
cp -R "$APP" "$STAGE/"
mkdir -p dist
DMG="dist/$DMG_NAME"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
[[ -f "$DMG" ]] || fail "DMG не создан: $DMG"
hdiutil verify "$DMG"
echo "==> SHA-256:"
shasum -a 256 "$DMG"
ls -lh "$DMG"
echo "Готово: $ROOT/$DMG"
