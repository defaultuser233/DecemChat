//! Decem Chat 桌面版入口。
//!
//! 无参数启动图形界面；`--selftest [--live]` 则不开窗口，跑一遍存储与请求编排自检，
//! 供 CI 与排障使用（`--live` 会真的调用一次模型接口，需要配置 API Key）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod emoji;
#[cfg(test)]
mod flow_test;
mod fonts;
mod media;
mod selftest;
mod store;
mod theme;
mod ui;
mod worker;

use std::io::Write;
use std::path::PathBuf;

/// 窗口标题。
const APP_NAME: &str = "Decem Chat";

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--selftest") {
        std::process::exit(selftest::run(args.iter().any(|arg| arg == "--live")));
    }

    init_logging();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_inner_size([980.0, 720.0])
        .with_min_inner_size([620.0, 480.0]);
    if let Some(icon) = assets::window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(app::DecemApp::new(cc)?))),
    )
}

/// 日志同时写到标准错误与配置目录下的日志文件（GUI 版没有控制台，只能看文件）。
fn init_logging() {
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));

    if let Some(path) = log_path() {
        match std::fs::File::create(&path) {
            Ok(file) => {
                builder.target(env_logger::Target::Pipe(Box::new(Tee {
                    stderr: std::io::stderr(),
                    file,
                })));
                eprintln!("日志文件：{}", path.display());
            }
            Err(err) => eprintln!("无法创建日志文件 {}：{err}", path.display()),
        }
    }

    builder.init();
}

fn log_path() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("DECEMCHAT_LOG") {
        return Some(PathBuf::from(custom));
    }
    store::Store::open()
        .ok()
        .map(|store| store.root().join("decemchat.log"))
}

/// 同时写 stderr 与文件。
struct Tee {
    stderr: std::io::Stderr,
    file: std::fs::File,
}

impl Write for Tee {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let _ = self.stderr.write_all(buffer);
        let _ = self.file.write_all(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = self.stderr.flush();
        let _ = self.file.flush();
        Ok(())
    }
}
