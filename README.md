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
- **Characters per save**: each save lists the characters inside it (Dark Souls Remastered and Dark Souls III).
- **Copy a single character** between saves, or into another slot of the same save (Dark Souls III,
  experimental). Pick the character, the destination save and slot; your saves are backed up first,
  and the result is verified byte for byte before it is written. Dark Souls Remastered and Dark Souls II
  can list/copy whole saves only for now.
- Light, dark or follow-the-system theme.

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
`packaging/install-linux.sh` builds and installs a launcher entry for your user.

## Safety

Using mods online can get an account banned. Souls-like keeps modded play on separate save files and
turns every mod off in Vanilla mode, but it can't guarantee anything. Use it at your own risk.
This project is not affiliated with or endorsed by FromSoftware, Bandai Namco or Valve.

## License

MIT, see [LICENSE](LICENSE). Bundled Noto Sans fonts are under the SIL OFL 1.1, see
[assets/fonts/README.md](assets/fonts/README.md).
