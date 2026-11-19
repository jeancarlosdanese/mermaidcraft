//! MermaidCraft: native Mermaid diagram editor.

// Windows: the release app opens without a console window (mermaidcraft-cli stays a console app).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod highlight;
mod i18n;
mod paint;
mod platform;
mod settings;

use std::path::PathBuf;

fn usage() {
    println!("MermaidCraft {}\n\nUsage: mermaidcraft [file.mmd|file.mermaid|file.md] [--screenshot out.png]\n\nFor the command line and MCP server use mermaidcraft-cli.", env!("CARGO_PKG_VERSION"));
}

fn main() -> eframe::Result<()> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut screenshot: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--screenshot" => match args.next() {
                Some(p) => screenshot = Some(PathBuf::from(p)),
                None => {
                    eprintln!("--screenshot needs a file name");
                    std::process::exit(2);
                }
            },
            "-h" | "--help" => {
                usage();
                return Ok(());
            }
            "-V" | "--version" => {
                println!("mermaidcraft {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            // macOS passes a process serial number when launched from the Finder.
            s if s.starts_with("-psn") => {}
            s if s.starts_with('-') => {
                eprintln!("unknown option '{s}'");
                std::process::exit(2);
            }
            _ => files.push(PathBuf::from(a)),
        }
    }
    let shot_mode = screenshot.is_some();
    let (_guard, events) = platform::listen();

    let mut viewport = egui::ViewportBuilder::default().with_title("MermaidCraft").with_inner_size([1280.0, 800.0]).with_min_inner_size([720.0, 480.0]);
    // Linux: the desktop links the window to ai.storyteller.mermaidcraft.desktop (name and icon).
    #[cfg(target_os = "linux")]
    {
        viewport = viewport.with_app_id("ai.storyteller.mermaidcraft");
    }
    if let Ok(rgba) = mermaidcraft_engine::export::svg_to_rgba(include_str!("../assets/icon.svg"), 256) {
        viewport = viewport.with_icon(egui::IconData { rgba, width: 256, height: 256 });
    }
    if shot_mode {
        // A check must not steal focus from what the person is doing.
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if shot_mode {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }
    let opts = app::Options { screenshot, files, events };
    eframe::run_native("MermaidCraft", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, opts)))))
}
