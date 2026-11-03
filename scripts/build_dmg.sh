#!/bin/bash
set -e

APP_NAME="BatteryBar"
BINARY_NAME="battery-bar"
VERSION="0.1.0"
DMG_NAME="${APP_NAME}-${VERSION}.dmg"
SRC_BINARY="${1:-target/release/${BINARY_NAME}}"

echo "==> Packaging ${APP_NAME} using binary: ${SRC_BINARY}"

BUILD_DIR="target/bundle"
APP_DIR="${BUILD_DIR}/${APP_NAME}.app"
DMG_STAGE="${BUILD_DIR}/dmg_stage"

rm -rf "${BUILD_DIR}"
mkdir -p "${APP_DIR}/Contents/MacOS"
mkdir -p "${APP_DIR}/Contents/Resources"
mkdir -p "${DMG_STAGE}"

# Copy executable
cp "${SRC_BINARY}" "${APP_DIR}/Contents/MacOS/${BINARY_NAME}"
chmod +x "${APP_DIR}/Contents/MacOS/${BINARY_NAME}"

# Copy Info.plist and AppIcon
cp "assets/Info.plist" "${APP_DIR}/Contents/Info.plist"
if [ -f "assets/AppIcon.icns" ]; then
    cp "assets/AppIcon.icns" "${APP_DIR}/Contents/Resources/AppIcon.icns"
fi

# Prepare DMG staging directory
cp -r "${APP_DIR}" "${DMG_STAGE}/"
ln -s /Applications "${DMG_STAGE}/Applications"

echo "==> Building ${DMG_NAME}..."
rm -f "${DMG_NAME}"

if command -v create-dmg >/dev/null 2>&1; then
    create-dmg \
        --volname "${APP_NAME} Installer" \
        --window-pos 200 120 \
        --window-size 600 350 \
        --icon-size 100 \
        --icon "${APP_NAME}.app" 175 120 \
        --app-drop-link 425 120 \
        --hide-extension "${APP_NAME}.app" \
        "${DMG_NAME}" \
        "${DMG_STAGE}" || {
            if [ ! -f "${DMG_NAME}" ]; then
                echo "Fallback to native hdiutil..."
                hdiutil create -volname "${APP_NAME}" -srcfolder "${DMG_STAGE}" -ov -format UDZO "${DMG_NAME}"
            fi
        }
else
    hdiutil create -volname "${APP_NAME}" -srcfolder "${DMG_STAGE}" -ov -format UDZO "${DMG_NAME}"
fi

echo "==> Finished: $(ls -lh "${DMG_NAME}")"
