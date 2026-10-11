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

fn main() -> eframe::Result {
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
