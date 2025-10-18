#!/bin/bash

# Exit on any error
set -e

echo "Installing executable..."
if [ ! -f /usr/bin/arch-update-manager ] || ! cmp -s target/release/arch-update-manager /usr/bin/arch-update-manager; then
    sudo cp target/release/arch-update-manager /usr/bin/
fi

echo "Installing desktop file..."
sudo cp arch-update-manager.desktop /usr/share/applications/

echo "Installing policy kit file..."
sudo cp com.destbg.arch-update-manager.policy /usr/share/polkit-1/actions/

echo "Installing icons..."
if [ -d "icons" ]; then
    sudo cp -r icons/* /usr/share/icons/hicolor/
fi

echo "Updating icon cache..."
sudo gtk-update-icon-cache /usr/share/icons/hicolor/ || true

echo "Installation complete!"