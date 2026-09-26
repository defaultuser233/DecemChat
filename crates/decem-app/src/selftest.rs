//! 无界面自检：验证「数据能存进去、能读回来、请求体拼得对」。
//!
//! 这些是界面之外最容易出问题、又最难在 GUI 里观察的部分，
//! 因此单独做成可在 CI 与无显示器环境运行的入口。

use anyhow::{Context, Result};
use decem_core::{ChatRequest, Message, MessageRole, default_settings};

use crate::store::{Store, StoredConfig};

/// 运行自检并返回进程退出码（0 通过 / 2 失败）。
pub fn run(live: bool) -> i32 {
    match self_check(live) {
        Ok(report) => {
            println!("{report}");
            println!("自检通过");
            0
        }
        Err(err) => {
            eprintln!("自检失败：{err:#}");
            2
        }
    }
}

fn self_check(live: bool) -> Result<String> {
    let mut lines: Vec<String> = Vec::new();

    // 1. 存储往返
    let dir = std::env::temp_dir().join("decemchat-selftest");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::at(&dir)?;

    let mut config = StoredConfig::new(default_settings());
    config.settings.model = "kimi-k3".to_owned();
    config.api_key = Some("sk-selftest".to_owned());
    store.save_config(&config)?;
    let loaded = store.load_config().context("配置读回失败")?;
    anyhow::ensure!(loaded.settings.model == "kimi-k3", "配置内容不一致");
    anyhow::ensure!(
        loaded.api_key.as_deref() == Some("sk-selftest"),
        "API Key 未保留"
    );

    store.save_messages(&[Message::text("1", MessageRole::User, "嗷呜～", 0)])?;
    anyhow::ensure!(store.load_messages().len() == 1, "聊天记录读回失败");

    store.save_image("1", "data:image/png;base64,AAAA")?;
    anyhow::ensure!(store.load_image("1").is_some(), "图片读回失败");
    lines.push(format!("存储：{} 可用", store.root().display()));

    // 2. 请求编排（与网页版契约一致）
    let messages = vec![Message::text("1", MessageRole::User, "嗷呜～", 0)];
    let request = ChatRequest::new("qwen3.8-flash", &messages);
    let body = serde_json::to_value(request.to_upstream())?;
    anyhow::ensure!(body["stream"] == true, "请求体缺少 stream=true");
    anyhow::ensure!(body["max_tokens"] == 1500, "请求体缺少 max_tokens");
    anyhow::ensure!(body["enable_thinking"] == false, "请求体应关闭思考");
    anyhow::ensure!(body["messages"][0]["role"] == "system", "首条应为 system");
    let prompt = body["messages"][0]["content"].as_str().unwrap_or_default();
    anyhow::ensure!(prompt.contains("Decem"), "人格提示词缺失");
    lines.push(format!(
        "请求编排：{} 条消息，模型 {}，temperature {}",
        request.messages.len(),
        request.model,
        request
            .temperature
            .map_or_else(|| "未下发".to_owned(), |value| value.to_string())
    ));

    // 3. 中文字体
    match crate::fonts::find_font() {
        Some(path) => lines.push(format!("中文字体：{}", path.display())),
        None => lines.push(format!(
            "中文字体：未找到（界面会显示方块，可用 {} 指定字体文件）",
            crate::fonts::FONT_ENV
        )),
    }

    // 4. 接口地址与鉴权模式
    let api_url = crate::app::resolve_api_url(store.load_config().as_ref());
    lines.push(format!("接口地址：{api_url}"));
    if api_url.trim().is_empty() {
        // 直连服务商：必须有 Key
        lines.push(match decem_core::api_key_from_env() {
            Some(_) => "API Key：已配置（直连模式）".to_owned(),
            None => "API Key：未配置，直连模式会失败".to_owned(),
        });
    } else {
        // 走代理：Key 在服务器上
        lines.push(format!("API Key：客户端无需配置（由 {api_url} 保管）"));
    }

    // 5. 可选的真实请求（走配置里的接口地址：代理模式不需要 Key）
    if live {
        lines.push(format!("实时请求：{}", live_call(&store)?));
    }

    Ok(lines.join("\n"))
}

/// 真发一次请求，验证「客户端 → 接口 → SSE 解析」整条链路。
fn live_call(store: &Store) -> Result<String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("无法创建 tokio 运行时")?;

    let request = ChatRequest::new(
        "qwen3.8-flash",
        &[Message::text("1", MessageRole::User, "用一句话打个招呼", 0)],
    );

    let saved = store.load_config();
    let api_key = saved
        .as_ref()
        .and_then(|config| config.api_key.clone())
        .or_else(decem_core::api_key_from_env)
        .unwrap_or_default();
    // 与界面用同一套解析逻辑：默认走网页版服务端，因此不需要 Key
    let api_url = crate::app::resolve_api_url(saved.as_ref());

    runtime.block_on(async {
        let mut client = decem_core::Client::new(api_key)?;
        if !api_url.trim().is_empty() {
            client = client.with_api_url(api_url);
        }
        let reply = client.complete(&request).await?;
        Ok::<String, anyhow::Error>(reply.trim().to_owned())
    })
}
