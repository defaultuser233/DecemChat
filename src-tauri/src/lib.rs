//! Decem Chat 的 Tauri 外壳。
//!
//! 窗口**不加载** Tauri 自己的资源，而是加载本机的一个小服务：它同时提供前端构建产物
//! 与 `/api/chat` 代理。这样前端一行都不用改——相对路径 `/api/chat` 直接命中它，
//! 同源、无 CORS、也不需要 IPC。

mod proxy;
mod server;

use std::sync::Arc;

use tauri::{WebviewUrl, WebviewWindowBuilder};

/// 上游接口地址默认值：本项目自己的服务端（Key 留在服务器上，客户端不需要填）。
pub const DEFAULT_API_URL: &str = "https://api.valedecem.top:8443/api/chat";

/// 覆盖上游接口地址的环境变量。
pub const API_URL_ENV: &str = "DECEMCHAT_API_URL";

/// 窗口标题。
const APP_NAME: &str = "Decem Chat";

/// 本地服务监听地址：交给系统分配空闲端口，避免和别的东西撞车。
const BIND_ADDR: &str = "127.0.0.1:0";

/// 解析上游地址：环境变量优先，否则用内置默认值。
pub fn resolve_api_url() -> String {
    std::env::var(API_URL_ENV)
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_API_URL.to_owned())
}

/// 启动外壳：先起本地服务拿到端口，再按该端口开窗。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let upstream = resolve_api_url();

    tauri::Builder::default()
        .setup(move |app| {
            let listener = tauri::async_runtime::block_on(async {
                tokio::net::TcpListener::bind(BIND_ADDR).await
            })?;
            let port = listener.local_addr()?.port();

            let state = Arc::new(server::AppState::new(upstream.clone()));
            tauri::async_runtime::spawn(async move {
                if let Err(err) = axum::serve(listener, server::router(state)).await {
                    eprintln!("本地服务退出：{err}");
                }
            });
            eprintln!("本地服务：http://127.0.0.1:{port}");
            eprintln!("上游接口：{upstream}");

            let url: tauri::Url = format!("http://127.0.0.1:{port}").parse()?;
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title(APP_NAME)
                .inner_size(980.0, 720.0)
                .min_inner_size(620.0, 480.0)
                .build()?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}
