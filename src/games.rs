pub struct Mod {
    pub id: &'static str,
    pub title: &'static str,
    pub sub: &'static str,
}

pub struct Summary {
    pub entry: &'static str,
    pub first: usize,
    pub stride: usize,
    pub lead: usize,
    pub table: Option<usize>,
}

pub struct Slots {
    pub count: usize,
    pub key: [u8; 16],
    pub entries: fn(usize) -> Vec<String>,
    pub summaries: &'static [Summary],
}

pub struct Game {
    pub key: &'static str,
    pub name: &'static str,
    pub sub: &'static str,
    pub appid: u32,
    pub dir: &'static [&'static str],
    pub exe: &'static str,
    pub sc: &'static str,
    pub roaming: bool,
    pub save_rel: &'static str,
    pub save_file: &'static str,
    pub backups: &'static str,
    pub accent: [u8; 3],
    pub mods: &'static [Mod],
    pub labels: &'static [(&'static str, &'static str)],
    pub slots: Slots,
}

impl Game {
    pub fn label(&self, ext: &str) -> &'static str {
        self.labels.iter().find(|(e, _)| *e == ext).map_or("Other", |l| l.1)
    }
}

fn one(i: usize) -> Vec<String> {
    vec![format!("USER_DATA{i:03}")]
}

fn two(i: usize) -> Vec<String> {
    vec![format!("USER_DATA{:03}", i + 1), format!("USER_DATA{:03}", i + 11)]
}

pub static GAMES: [Game; 3] = [
    Game {
        key: "ds1",
        name: "Dark Souls",
        sub: "Remastered",
        appid: 570940,
        dir: &["DARK SOULS REMASTERED"],
        exe: "DarkSoulsRemastered.exe",
        sc: "ds1sc",
        roaming: false,
        save_rel: "NBGI/DARK SOULS REMASTERED",
        save_file: "DRAKS0005",
        backups: "DS1R Save Backups",
        accent: [200, 85, 61],
        mods: &[],
        labels: &[("sl2", "Normal game"), ("co2", "Seamless Co-op")],
        slots: Slots {
            count: 10,
            key: [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32, 0x10],
            entries: one,
            summaries: &[Summary { entry: "USER_DATA010", first: 212, stride: 400, lead: 6, table: None }],
        },
    },
    Game {
        key: "ds2",
        name: "Dark Souls II",
        sub: "Scholar of the First Sin",
        appid: 335300,
        dir: &["Dark Souls II Scholar of the First Sin", "Game"],
        exe: "DarkSoulsII.exe",
        sc: "ds2sc",
        roaming: true,
        save_rel: "DarkSoulsII",
        save_file: "DS2SOFS0000",
        backups: "DS2 Save Backups",
        accent: [93, 138, 168],
        mods: &[Mod { id: "rando", title: "Randomizer", sub: "Shuffles items and enemies. Uses its own save (.co2); off = plain Seamless (.va2)" }],
        labels: &[("sl2", "Normal game"), ("va2", "Seamless Co-op (vanilla)"), ("co2", "Seamless Co-op (randomizer)"), ("cov", "Old test save")],
        slots: Slots {
            count: 10,
            key: [0x59, 0x9F, 0x9B, 0x69, 0x96, 0x40, 0xA5, 0x52, 0x36, 0xEE, 0x2D, 0x70, 0x83, 0x5E, 0xC7, 0x44],
            entries: two,
            summaries: &[Summary { entry: "USER_DATA000", first: 1306, stride: 496, lead: 52, table: None }, Summary { entry: "USER_DATA022", first: 462, stride: 496, lead: 52, table: None }],
        },
    },
    Game {
        key: "ds3",
        name: "Dark Souls III",
        sub: "",
        appid: 374320,
        dir: &["DARK SOULS III", "Game"],
        exe: "DarkSoulsIII.exe",
        sc: "ds3sc",
        roaming: true,
        save_rel: "DarkSoulsIII",
        save_file: "DS30000",
        backups: "DS3 Save Backups",
        accent: [176, 141, 87],
        mods: &[Mod { id: "honest", title: "Honest Merchant", sub: "Item and gear shop from the Firelink corpse. Changes the game itself, so use it with co-op only" }],
        labels: &[("sl2", "Normal game"), ("co2", "Seamless Co-op")],
        slots: Slots {
            count: 10,
            key: [0xFD, 0x46, 0x4D, 0x69, 0x5E, 0x69, 0xA3, 0x9A, 0x10, 0xE3, 0x19, 0xA7, 0xAC, 0xE8, 0xB7, 0xFA],
            entries: one,
            summaries: &[Summary { entry: "USER_DATA010", first: 4274, stride: 554, lead: 0x20, table: Some(0x20) }],
        },
    },
];
