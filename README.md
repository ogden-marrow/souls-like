# Souls-like

A small, dark-mode-ready launcher and save manager for **Dark Souls Remastered**, **Dark Souls II
(Scholar of the First Sin)** and **Dark Souls III**, built around the **Seamless Co-op** mods.

Written in Rust with [egui](https://github.com/emilk/egui). One binary, no runtime.

## What it does

- **One-click launch** per game, in two modes:
  - **Vanilla** switches every mod loader off (`dinput8.dll` is renamed) and starts the plain game
    through Steam, on your normal `.sl2` save.
  - **Seamless Co-op** starts the mod with its own save file. Souls-like refuses to start a modded
    session on `.sl2`, and backs your saves up before every launch.
- **Mod toggles** (DS2 randomizer, DS3 Honest Merchant) and a co-op password field.
- **Drag-and-drop mod install.** Drop a mod `.zip` on the window; the game and mod are recognised
  from marker files inside it (`ds1sc_launcher.exe`, `ds2sc_launcher.exe`, `ds3sc_launcher.exe`,
  `honestMerchant.ini`). Replaced files are backed up, your existing settings files are kept, and
  zips with unsafe paths are rejected.
- **Shared co-op settings**: set the password, invaders, scaling, etc. once and mirror them into
  every game's Seamless Co-op settings file (per-setting opt-out; the save extension is never mirrored).
- **Save manager**: back up, restore, copy one save slot over another, export, and trim the mod's
  auto-backups. Anything that overwrites a save backs it up first.
- Game icons are read from each game's own `.exe` at runtime (cached locally); no game art is bundled.
- **Characters per save**: each save lists the characters inside it, for all three games.
- **Copy a single character** between saves, or into another slot of the same save, in Dark Souls
  Remastered, **Dark Souls II** and Dark Souls III. See [Copying characters](#copying-characters).
- **Folder pickers** in Settings for each game's install folder and save folder (blank = detect from Steam).
- Light, dark or follow-the-system theme.

## Copying characters

Open a game, find the save the character is in and press **Copy character…**. Pick the character, the
destination save (it can be the same save) and the slot. It defaults to the first empty slot; if you
pick a used slot you are warned before it is replaced.

| Game | Supported | How it is stored |
|---|---|---|
| Dark Souls Remastered | yes | One slot entry per character, plus a 400-byte block per slot in the profile summary (`USER_DATA010`). |
| **Dark Souls II (SotFS)** | **yes** | Each character is two entries (`USER_DATA00n` and `USER_DATA01n`), plus a 496-byte block per slot in **both** profile summaries (`USER_DATA000` and `USER_DATA022`). All four pieces are moved together. |
| Dark Souls III | yes | One slot entry per character, plus a 554-byte block per slot in the profile summary (`USER_DATA010`). |

What happens when you confirm:

1. The current saves are backed up (Documents / "<game> Save Backups").
2. The character's encrypted data is copied across untouched, and the matching summary blocks are
   updated and re-encrypted.
3. The new file is rebuilt in memory and checked byte for byte (the moved character equals its source,
   every other slot is unchanged, the character list shows the copied character). Only then is it
   written, through a temporary file, so a failure can never leave a half-written save.

This is **experimental**: it has been verified against real saves, but try it on a throwaway copy first
and keep the automatic backups. Close the game before copying. Both saves must belong to the same Steam
account.

## Platforms

| OS | Status |
|---|---|
| Linux | Works. Games run through Steam's Proton; Steam (including the snap and Flatpak installs) is found automatically. |
| Windows | Games run natively. Written to the same design, but **not yet tested on a real Windows machine**. |
| macOS | Compiles, but there is no game runner yet (the games have no Mac build). Save management works if you point the save folder at one under Settings. |

## Mods are not included

This project does not ship or redistribute any mods. Get Seamless Co-op and Honest Merchant from
their authors and drop the zips into the app.

## Build

```sh
cargo build --release
./target/release/souls-like
```

On Linux you need the usual windowing libraries (`libxkbcommon`, `libwayland`, `libx11`, OpenGL).
On Linux, run `./souls-like --install` once (from the download or after `cargo build --release`) to copy it to `~/.local/bin` and add an app-menu entry.
Every release is built by GitHub Actions with a signed build-provenance attestation: `gh attestation verify <zip> --repo ogden-marrow/souls-like`.

## Safety

Using mods online can get an account banned. Souls-like keeps modded play on separate save files and
turns every mod off in Vanilla mode, but it can't guarantee anything. Use it at your own risk.
This project is not affiliated with or endorsed by FromSoftware, Bandai Namco or Valve.

## License

MIT, see [LICENSE](LICENSE). Bundled Noto Sans fonts are under the SIL OFL 1.1, see
[assets/fonts/README.md](assets/fonts/README.md).

## Antivirus warnings

The Windows exe is not code-signed, and unsigned programs that start other programs or edit game files are sometimes flagged by heuristic scanners.
Souls-like only launches Steam and the game, reads and writes your saves and the mod folders, and makes no network connections.
Compare the zip's SHA-256 with `SHA256SUMS.txt` on the release page, or build it yourself from this repository.
