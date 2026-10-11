mod app;
mod config;
mod games;
mod ini;
mod launch;
mod modinstall;
mod platform;
mod saves;
mod shared;

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Souls-like")
            .with_app_id("souls-like")
            .with_inner_size([1060.0, 740.0])
            .with_min_inner_size([860.0, 560.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Souls-like",
        opts,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
