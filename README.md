# Arch Update Manager

A Linux Mint inspired GTK4-based update manager for Arch Linux.

![Arch Update Manager](showcase.png)

## Features

- View available package updates from pacman, AUR, and Flatpak
- Select or deselect packages for installation
- Create a Timeshift or Snapper snapshot before updates
- System tray icon with update notifications, started by systemd
- Mark packages as favorites and keep them at the top of the list
- Blacklist packages so they are hidden from the update list
- Sort packages and group them by repository or source
- Switch a package between repositories, for example aur to extra
- Remember packages you unselected so they stay unselected next time
- Post update checks for service restarts and pacnew files
- Pacman cache cleanup with paccache
- Settings dialog to control AUR helper, Flatpak, snapshots, and the tray

## Installing

```bash
makepkg -si
```
Or using the AUR
```bash
paru arch-update-manager
```

There are three AUR packages:
- `arch-update-manager` builds from the latest tagged release
- `arch-update-manager-bin` installs the prebuilt binary from that release
- `arch-update-manager-git` builds from the latest commit on main

## License

This project is licensed under the MIT License.