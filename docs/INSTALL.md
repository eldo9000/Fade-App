# Installing Fade

A short guide to getting Fade running on macOS. If you're looking for
release engineering details (signing keys, CI, the updater manifest),
see [`RELEASE.md`](./RELEASE.md) instead.

---

## Requirements

- **macOS 13 (Ventura) or newer**
- **Apple Silicon or Intel** (x64 and aarch64 both supported)
- **ffmpeg** and **imagemagick** — required for most conversions:

  ```sh
  brew install ffmpeg imagemagick
  ```

---

## Optional tools

Fade detects these at runtime. If they're missing, the relevant feature shows a clear error — nothing breaks.

| Tool | Feature | Install |
|------|---------|---------|
| `libreoffice` | Office doc conversion (DOCX, XLSX, PPTX, iWork) | [libreoffice.org](https://www.libreoffice.org) or `brew install --cask libreoffice` |
| `blender` | 3D format conversion (USD, USDZ, Alembic, .blend) | [blender.org](https://www.blender.org) — requires Blender 3.0+; USDZ requires 3.5+ |
| `freecad` | CAD format conversion (STEP, IGES) | [freecad.org](https://www.freecad.org) |
| `HandBrakeCLI` | DVD / Blu-ray rip | [handbrake.fr](https://handbrake.fr/downloads2.php) |
| `dvdauthor` + `mkisofs` | DVD authoring | `brew install dvdauthor dvdauthor cdrtools` |
| `dcraw` | Camera RAW input (CR2, NEF, ARW, DNG, etc.) | `brew install dcraw` |
| `cjxl` / `djxl` | JPEG XL conversion | `brew install jpeg-xl` |
| Python + `demucs` | Audio stem separation | `pip install demucs` |
| Python + `openai-whisper` | Speech transcription | `pip install openai-whisper` |
| Python + `argostranslate` | Subtitle/text translation | `pip install argostranslate` |
| Python + `ddcolor` | Video/image colorization | `pip install ddcolor` |
| Python + `rembg` | Background removal | `pip install rembg` |
| Python + `robust-video-matting` | Neural video matting | `pip install robust-video-matting` |

---

## Install

1. Go to the [latest release page](https://github.com/eldo9000/Fade-App/releases/latest).
2. Download the `.dmg` file (named something like `Fade_0.2.0_aarch64.dmg`).
3. Open the DMG and drag **Fade** into your **Applications** folder.
4. Eject the DMG.

### First launch (Gatekeeper)

Fade isn't signed with an Apple Developer ID, so macOS will block the
first launch with a message like:

> "Fade" is damaged and can't be opened.

or

> "Fade" cannot be opened because Apple cannot check it for malicious
> software.

This is expected. Pick **one** of the two workarounds below.

**Option A — Terminal (fastest):**

```sh
xattr -cr /Applications/Fade.app
```

Then double-click Fade normally. This removes the quarantine flag macOS
adds to downloaded apps.

**Option B — Right-click → Open:**

1. Open your Applications folder in Finder.
2. Right-click (or Control-click) **Fade**.
3. Choose **Open** from the menu.
4. In the dialog, click **Open** again.

You only need to do this once. After the first successful launch, Fade
opens like any other app.

---

## Updates

Fade updates itself. A couple of seconds after launch, it checks GitHub
for a newer release. If one exists, a banner appears at the top of the
window with an **Install & restart** button. Click it and Fade downloads
the update, verifies its signature, swaps itself out, and relaunches.

You don't need to re-download the DMG or redo the Gatekeeper workaround
for updates — only the very first install.

To check which version you're on, open Fade's menu (the application menu
next to the Apple logo) → **About Fade**.

---

## Uninstall

Drag `/Applications/Fade.app` to the Trash. That's it. Fade doesn't
install anything outside the app bundle.

If you also want to remove ffmpeg and imagemagick:

```sh
brew uninstall ffmpeg imagemagick
```

---

## Troubleshooting

**"command not found: ffmpeg" or conversion fails immediately.**
You don't have ffmpeg (or imagemagick) on your PATH. Install via
Homebrew as shown in [Requirements](#requirements). If they're installed
but Fade still can't find them, your shell PATH probably doesn't include
`/opt/homebrew/bin`. Open Terminal and run `which ffmpeg` to confirm.

**Gatekeeper still blocks Fade after running `xattr -cr`.**
Re-run it with `sudo`:

```sh
sudo xattr -cr /Applications/Fade.app
```

If you moved the app somewhere other than `/Applications`, adjust the
path accordingly.

**Fade launches then immediately quits, or the window never appears.**
Open **Console.app** (press Cmd+Space, type "Console"), click **Crash
Reports** in the sidebar, and look for a recent `Fade` entry. The top
few lines usually identify the problem. If you can't make sense of the
report, [open an issue](https://github.com/eldo9000/Fade-App/issues) and
paste the first ~20 lines.

**The updater banner never appears even though there's a new release.**
The check happens ~2 seconds after launch and requires internet. Quit
Fade completely (Cmd+Q — not just closing the window) and reopen. If
you're on a restrictive network, GitHub may be blocked.

**"The application can't be opened" after an update.**
Rare, but if an update leaves Fade in a broken state, delete
`/Applications/Fade.app` and reinstall from the
[latest release](https://github.com/eldo9000/Fade-App/releases/latest).
Your settings are preserved.
