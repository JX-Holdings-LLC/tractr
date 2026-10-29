# Install and update Tractr

Tractr is a desktop app for Windows, Linux and macOS. Download it from the
[latest release](https://github.com/JX-Holdings-LLC/tractr/releases/latest);
it updates itself after that.

## Windows

1. Download `Tractr-win-Setup.exe` and run it. It installs for your user
   only and needs no admin rights; it adds Start-menu and desktop shortcuts.
2. The installer is not code-signed yet, so Windows SmartScreen may say
   *"Windows protected your PC"*. Choose **More info → Run anyway**. Updates
   installed later don't ask again.

Prefer no installer? `Tractr-win-Portable.zip` is a portable copy: unzip it
anywhere and run `Tractr.exe`. It updates itself too.

## Linux

1. Download `Tractr-linux-x64.AppImage` into a folder you can write to.
2. Make it executable and run it:

   ```sh
   chmod +x Tractr-linux-x64.AppImage
   ./Tractr-linux-x64.AppImage
   ```

Tractr needs a graphical desktop and, on Ubuntu 22.04 or newer (the build
baseline), these packages:

```sh
sudo apt-get install libasound2 libudev1 libxkbcommon-x11-0 libfuse2
```

On Ubuntu 24.04 the first and last are called `libasound2t64` and
`libfuse2t64`. Without ALSA (`libasound2`) Tractr still runs, without sound.

## macOS

1. Download `Tractr-osx-arm64-Portable.zip` for Apple Silicon or
   `Tractr-osx-x64-Portable.zip` for an Intel Mac.
2. Unzip it and move `Tractr.app` to Applications.
3. Until the Mac app is signed and notarized, macOS blocks the first launch:
   open **System Settings → Privacy & Security** and choose **Open Anyway**
   for Tractr.

The Mac editor uses Command where this documentation says Ctrl. It exports
Windows, Linux and web games; a macOS game target is not available yet.

## Updates

**Help → Update Center** checks for newer versions, shows what changed,
downloads the update in the background and applies it with **Restart &
install** (or when you next close Tractr). It can:

- follow the **Stable** channel, or **Pre-release** for early versions;
- **Check for updates on startup** (on by default, at most every 6 hours);
- **Download updates automatically** (off by default);
- **Skip this version**.

![The Update Center with release notes and a download button.](media/update-center.png)

Your projects are not touched by an update. A copy of Tractr built from
source, or run from an IDE, can't update itself: the Update Center says
*"Updates are available only in the installed app"* and links to the
installer.

## Where Tractr keeps things

| | Windows | Linux | macOS |
|---|---|---|---|
| Projects | `%APPDATA%\Tractr\projects\` | `~/.config/Tractr/projects/` | `~/Library/Application Support/Tractr/projects/` |
| Settings | `%APPDATA%\Tractr\settings.json` | `~/.config/Tractr/settings.json` | `~/Library/Application Support/Tractr/settings.json` |
| The app (installed) | `%LocalAppData%\Tractr` | where you put the AppImage | where you put `Tractr.app` |

Set `TRACTR_DATA_DIR` to keep projects and settings in another folder (a
USB stick, say). The app's `licenses` folder holds the notices of the
software it includes, and **Help → About Tractr** links to Tractr's license
([LICENSING.md](../LICENSING.md)).

## Check a download

Each release lists `SHA256SUMS` and carries a build provenance attestation:

```sh
sha256sum --check --ignore-missing SHA256SUMS
gh attestation verify Tractr-win-Setup.exe --repo JX-Holdings-LLC/tractr
```

## Uninstall

On Windows, remove Tractr in **Settings → Apps**. On Linux and macOS, delete
the AppImage or `Tractr.app`. Your projects stay in the folders above until
you delete them.
