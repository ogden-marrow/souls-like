//! Static description of the supported games.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SaveBase {
    /// `Documents/<rel>`
    Documents,
    /// `%APPDATA%/<rel>` (Roaming)
    Roaming,
}

pub struct Mod {
    pub id: &'static str,
    pub title: &'static str,
    pub sub: &'static str,
}

pub struct Game {
    pub key: &'static str,
    pub name: &'static str,
    pub sub: &'static str,
    pub appid: u32,
    /// Game folder under `steamapps/common` (the folder holding the exe and mod files).
    pub dir: &'static [&'static str],
    pub proc_name: &'static str,
    /// Seamless Co-op file prefix: `<sc>_launcher.exe`, `SeamlessCoop/<sc>_settings.ini`.
    pub sc: &'static str,
    pub save_base: SaveBase,
    pub save_rel: &'static str,
    pub save_file: &'static str,
    pub backups: &'static str,
    pub accent: [u8; 3],
    pub mods: &'static [Mod],
    pub labels: &'static [(&'static str, &'static str)],
}

impl Game {
    pub fn label(&self, ext: &str) -> &'static str {
        self.labels
            .iter()
            .find(|(e, _)| *e == ext)
            .map(|(_, l)| *l)
            .unwrap_or("Other")
    }
}

pub static GAMES: [Game; 3] = [
    Game {
        key: "ds1",
        name: "Dark Souls",
        sub: "Remastered",
        appid: 570940,
        dir: &["DARK SOULS REMASTERED"],
        proc_name: "DarkSoulsRemastered.exe",
        sc: "ds1sc",
        save_base: SaveBase::Documents,
        save_rel: "NBGI/DARK SOULS REMASTERED",
        save_file: "DRAKS0005",
        backups: "DS1R Save Backups",
        accent: [200, 85, 61],
        mods: &[],
        labels: &[("sl2", "Normal game"), ("co2", "Seamless Co-op")],
    },
    Game {
        key: "ds2",
        name: "Dark Souls II",
        sub: "Scholar of the First Sin",
        appid: 335300,
        dir: &["Dark Souls II Scholar of the First Sin", "Game"],
        proc_name: "DarkSoulsII.exe",
        sc: "ds2sc",
        save_base: SaveBase::Roaming,
        save_rel: "DarkSoulsII",
        save_file: "DS2SOFS0000",
        backups: "DS2 Save Backups",
        accent: [93, 138, 168],
        mods: &[Mod {
            id: "rando",
            title: "Randomizer",
            sub: "Shuffles items and enemies. Uses its own save (.co2); off = plain Seamless (.va2)",
        }],
        labels: &[
            ("sl2", "Normal game"),
            ("va2", "Seamless Co-op (vanilla)"),
            ("co2", "Seamless Co-op (randomizer)"),
            ("cov", "Old test save"),
        ],
    },
    Game {
        key: "ds3",
        name: "Dark Souls III",
        sub: "",
        appid: 374320,
        dir: &["DARK SOULS III", "Game"],
        proc_name: "DarkSoulsIII.exe",
        sc: "ds3sc",
        save_base: SaveBase::Roaming,
        save_rel: "DarkSoulsIII",
        save_file: "DS30000",
        backups: "DS3 Save Backups",
        accent: [176, 141, 87],
        mods: &[Mod {
            id: "honest",
            title: "Honest Merchant",
            sub: "Item and gear shop from the Firelink corpse. Changes the game itself, so use it with co-op only",
        }],
        labels: &[("sl2", "Normal game"), ("co2", "Seamless Co-op")],
    },
];
