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

/// How character slots are stored in a save (BND4 `USER_DATA...` entries), for games where it is understood.
pub struct SlotLayout {
    pub count: usize,
    pub key: [u8; 16],
    pub names: NameSource,
    /// The save entries that belong to one character slot (moved together when copying).
    pub slot_entries: fn(usize) -> Vec<String>,
    /// The profile-summary tables that describe each slot; copying a character updates all of them.
    /// Empty = copying single characters isn't supported for this game.
    pub summaries: &'static [Summary],
}

/// Where a character's name lives.
#[derive(Clone, Copy)]
pub enum NameSource {
    /// At a fixed byte offset inside the character's own slot data.
    InSlot(usize),
    /// In a profile summary entry: `first` is the name of slot 0, then `stride` bytes per slot.
    Summary {
        entry: &'static str,
        first: usize,
        stride: usize,
    },
}

/// A table with one fixed-size block per slot.
pub struct Summary {
    pub entry: &'static str,
    /// Offset of slot 0's character name inside the entry.
    pub first: usize,
    pub stride: usize,
    /// A slot's block starts this many bytes before its name.
    pub lead: usize,
    /// Optional per-slot byte table at this offset (one byte per slot).
    pub table: Option<usize>,
}

fn one_entry(i: usize) -> Vec<String> {
    vec![format!("USER_DATA{i:03}")]
}

/// Dark Souls II: slot `i` is `USER_DATA{i+1}` plus its companion `USER_DATA{i+11}`.
fn ds2_entries(i: usize) -> Vec<String> {
    vec![
        format!("USER_DATA{:03}", i + 1),
        format!("USER_DATA{:03}", i + 11),
    ]
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
    pub slots: Option<SlotLayout>,
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
        slots: Some(SlotLayout {
            count: 10,
            key: [
                0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
                0x32, 0x10,
            ],
            names: NameSource::Summary {
                entry: "USER_DATA010",
                first: 212,
                stride: 400,
            },
            slot_entries: one_entry,
            summaries: &[Summary {
                entry: "USER_DATA010",
                first: 212,
                stride: 400,
                lead: 6,
                table: None,
            }],
        }),
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
        slots: Some(SlotLayout {
            count: 10,
            key: [
                0x59, 0x9F, 0x9B, 0x69, 0x96, 0x40, 0xA5, 0x52, 0x36, 0xEE, 0x2D, 0x70, 0x83, 0x5E,
                0xC7, 0x44,
            ],
            names: NameSource::Summary {
                entry: "USER_DATA000",
                first: 1306,
                stride: 496,
            },
            slot_entries: ds2_entries,
            summaries: &[
                Summary {
                    entry: "USER_DATA000",
                    first: 1306,
                    stride: 496,
                    lead: 52,
                    table: None,
                },
                Summary {
                    entry: "USER_DATA022",
                    first: 462,
                    stride: 496,
                    lead: 52,
                    table: None,
                },
            ],
        }),
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
        slots: Some(SlotLayout {
            count: 10,
            key: [
                0xFD, 0x46, 0x4D, 0x69, 0x5E, 0x69, 0xA3, 0x9A, 0x10, 0xE3, 0x19, 0xA7, 0xAC, 0xE8,
                0xB7, 0xFA,
            ],
            names: NameSource::Summary {
                entry: "USER_DATA010",
                first: 4274,
                stride: 554,
            },
            slot_entries: one_entry,
            summaries: &[Summary {
                entry: "USER_DATA010",
                first: 4274,
                stride: 554,
                lead: 0x20,
                table: Some(0x20),
            }],
        }),
    },
];
