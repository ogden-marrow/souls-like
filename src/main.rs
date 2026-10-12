#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod bnd4;
mod config;
mod gameicon;
mod games;
mod ini;
mod launch;
mod modinstall;
mod platform;
mod saves;
mod shared;
mod slots;

#[cfg(target_os = "linux")]
fn install() -> std::io::Result<()> {
    let home = dirs::home_dir().unwrap_or_default();
    let (bin, icon, entry) = (home.join(".local/bin/souls-like"), home.join(".local/share/icons/hicolor/256x256/apps/souls-like.png"), home.join(".local/share/applications/souls-like.desktop"));
    for p in [&bin, &icon, &entry] {
        std::fs::create_dir_all(p.parent().unwrap())?;
    }
    std::fs::copy(std::env::current_exe()?, &bin)?;
    std::fs::write(&icon, include_bytes!("../assets/icon.png"))?;
    std::fs::write(
        &entry,
        format!(
            "[Desktop Entry]\nType=Application\nName=Souls-like\nComment=Launcher and save manager for Dark Souls 1, 2 and 3\nExec={}\nIcon={}\nCategories=Game;\nTerminal=false\nStartupWMClass=souls-like\nStartupNotify=false\n",
            bin.display(),
            icon.display()
        ),
    )
}

fn main() -> eframe::Result {
    #[cfg(target_os = "linux")]
    if std::env::args().any(|a| a == "--install") {
        match install() {
            Ok(()) => println!("Installed. Launch Souls-like from your app menu."),
            Err(e) => eprintln!("Install failed: {e}"),
        }
        return Ok(());
    }
    let icon = image::load_from_memory(include_bytes!("../assets/icon.png")).expect("icon").to_rgba8();
    let viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Souls-like")
        .with_app_id("souls-like")
        .with_inner_size([1060.0, 740.0])
        .with_min_inner_size([860.0, 560.0])
        .with_icon(eframe::egui::IconData { width: icon.width(), height: icon.height(), rgba: icon.into_raw() });
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Souls-like", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
