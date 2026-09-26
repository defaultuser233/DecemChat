//! 端到端流程测试：输入 → 请求 → SSE 流 → 打字机 → 落盘。
//!
//! 用一个本地假上游（普通 TCP，返回 SSE）顶替 DashScope，
//! 因此不依赖网络与 API Key，CI 里也能跑。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use decem_core::{ChatSettings, MessageRole, default_settings};

use crate::app::DecemApp;
use crate::fonts::LoadedFonts;
use crate::store::{Store, StoredConfig};
use crate::ui::Action;

/// 本地假上游：接受一次连接，返回固定 SSE 后关闭。
struct MockUpstream {
    url: String,
    _thread: std::thread::JoinHandle<()>,
}

impl MockUpstream {
    fn start(chunks: &[&str]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本地端口失败");
        let port = listener.local_addr().expect("读取本地端口失败").port();
        let body = sse_body(chunks);

        let thread = std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                let _ = serve(stream, &body);
            }
        });

        Self {
            url: format!("http://127.0.0.1:{port}/compatible-mode/v1/chat/completions"),
            _thread: thread,
        }
    }
}

fn sse_body(chunks: &[&str]) -> String {
    let mut body = String::new();
    for chunk in chunks {
        let text = serde_json::to_string(chunk).expect("序列化文本失败");
        body.push_str(&format!(
            "data: {{\"choices\":[{{\"delta\":{{\"content\":{text}}}}}]}}\n\n"
        ));
    }
    body.push_str("data: [DONE]\n\n");
    body
}

/// 读掉请求（含 Content-Length 指定的正文），再回 SSE。
fn serve(mut stream: TcpStream, body: &str) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut content_length = 0usize;

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if let Some(value) = line
            .to_ascii_lowercase()
            .strip_prefix("content-length:")
            .map(str::to_owned)
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
        if line == "\r\n" {
            break;
        }
    }

    if content_length > 0 {
        let mut request_body = vec![0u8; content_length];
        reader.read_exact(&mut request_body)?;
    }

    let response = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: text/event-stream; charset=utf-8\r\n\
         Cache-Control: no-cache\r\n\
         Connection: close\r\n\r\n{body}"
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

pub(crate) fn temp_store(tag: &str) -> Store {
    let dir = std::env::temp_dir().join(format!("decemchat-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Store::at(&dir).expect("创建测试目录失败")
}

#[test]
fn chat_flow_reaches_the_message_list_and_the_disk() {
    let upstream = MockUpstream::start(&["🦊 ", "嗷呜～"]);
    let store = temp_store("flow");

    let mut config = StoredConfig::new(ChatSettings {
        model: "qwen3.8-flash".to_owned(),
        ..default_settings()
    });
    config.api_key = Some("sk-test".to_owned());
    config.api_url = Some(upstream.url.clone());
    store.save_config(&config).expect("写入测试配置失败");

    let ctx = egui::Context::default();
    let mut app =
        DecemApp::build(store.clone(), &ctx, LoadedFonts::default(), None).expect("构造应用失败");

    app.input = "你好呀".to_owned();
    app.apply(Action::Send);
    assert!(app.is_busy(), "发送后应进入等待 / 打字状态");

    let deadline = Instant::now() + Duration::from_secs(15);
    while app.is_busy() && Instant::now() < deadline {
        app.pump_worker();
        app.pump_typing(&ctx);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!app.is_busy(), "应在超时前完成回复");
    app.flush_messages();

    let rendered: Vec<(MessageRole, String)> = app
        .messages
        .iter()
        .map(|message| (message.role, message.content.clone()))
        .collect();
    assert!(
        rendered
            .iter()
            .any(|(role, text)| *role == MessageRole::User && text == "你好呀"),
        "用户消息应进入列表：{rendered:?}"
    );

    let reply = app.messages.last().expect("应有一条回复");
    assert_eq!(reply.role, MessageRole::Assistant);
    assert_eq!(reply.content, "🦊 嗷呜～", "打字机结束后应显示完整回复");

    let saved = store.load_messages();
    assert!(
        saved.iter().any(|message| message.content == "🦊 嗷呜～"),
        "回复应写入磁盘：{saved:?}"
    );
}

#[test]
fn upstream_failure_removes_the_placeholder_and_toasts() {
    let store = temp_store("flow-fail");
    let mut config = StoredConfig::new(default_settings());
    config.api_key = Some("sk-test".to_owned());
    // 指向一个必然连不上的端口
    config.api_url = Some("http://127.0.0.1:9/v1/chat/completions".to_owned());
    store.save_config(&config).expect("写入测试配置失败");

    let ctx = egui::Context::default();
    let mut app = DecemApp::build(store, &ctx, LoadedFonts::default(), None).expect("构造应用失败");

    app.input = "喂".to_owned();
    app.apply(Action::Send);

    let deadline = Instant::now() + Duration::from_secs(15);
    while app.is_busy() && Instant::now() < deadline {
        app.pump_worker();
        app.pump_typing(&ctx);
        std::thread::sleep(Duration::from_millis(5));
    }

    assert!(!app.is_busy(), "失败后应退出等待状态");
    assert!(
        app.messages
            .iter()
            .all(|message| !message.content.is_empty()),
        "失败时不应留下空的Decem 气泡：{:?}",
        app.messages
    );
    assert!(!app.toasts.is_empty(), "应给出错误提示");
}

/// 输入框里按 Enter 应当真的把消息发出去（覆盖「控件 → Action」这段接线）。
///
/// 用无头 `egui::Context` 跑一遍真实的界面绘制 + 真实按键事件，
/// 不需要窗口，因此 CI 里也能跑。
#[test]
fn pressing_enter_in_the_composer_sends_the_message() {
    let upstream = MockUpstream::start(&["收到啦～"]);
    let store = temp_store("flow-enter");

    let mut config = StoredConfig::new(ChatSettings {
        model: "qwen3.8-flash".to_owned(),
        ..default_settings()
    });
    config.api_key = Some("sk-test".to_owned());
    config.api_url = Some(upstream.url.clone());
    store.save_config(&config).expect("写入测试配置失败");

    let ctx = egui::Context::default();
    let mut app = DecemApp::build(store, &ctx, LoadedFonts::default(), None).expect("构造应用失败");
    app.input = "你好呀".to_owned();

    // 真实运行时由鼠标点击让输入框获得焦点，这里直接要求焦点
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("decem-composer-input")));

    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(980.0, 720.0),
        )),
        events: vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        ..Default::default()
    };

    let mut actions = Vec::new();
    // 无头运行没有渲染器，纹理增量需要显式清掉（否则 TexturesDelta 的 debug_assert 会 panic）
    let mut output = ctx.run_ui(raw_input, |ui| {
        actions = app.draw(ui);
    });
    output.textures_delta.clear();

    assert!(
        actions.contains(&Action::Send),
        "输入框聚焦时按 Enter 应产生发送动作：{actions:?}"
    );
    for action in actions {
        app.apply(action);
    }
    assert!(app.is_busy(), "发送后应进入等待状态");

    let deadline = Instant::now() + Duration::from_secs(15);
    while app.is_busy() && Instant::now() < deadline {
        app.pump_worker();
        app.pump_typing(&ctx);
        std::thread::sleep(Duration::from_millis(5));
    }

    assert_eq!(
        app.messages.last().map(|message| message.content.as_str()),
        Some("收到啦～"),
        "回复应显示在消息列表里：{:?}",
        app.messages
    );
}

/// 没有焦点时按 Enter 不应发送（避免误发）。
#[test]
fn enter_without_focus_does_not_send() {
    let store = temp_store("flow-nofocus");
    let ctx = egui::Context::default();
    let mut app = DecemApp::build(store, &ctx, LoadedFonts::default(), None).expect("构造应用失败");
    app.input = "还没写完".to_owned();

    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(980.0, 720.0),
        )),
        events: vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        ..Default::default()
    };

    let mut actions = Vec::new();
    // 无头运行没有渲染器，纹理增量需要显式清掉（否则 TexturesDelta 的 debug_assert 会 panic）
    let mut output = ctx.run_ui(raw_input, |ui| {
        actions = app.draw(ui);
    });
    output.textures_delta.clear();

    assert!(
        !actions.contains(&Action::Send),
        "未聚焦时不应发送：{actions:?}"
    );
    assert!(!app.is_busy());
}

/// 跑一帧，返回这一帧画出的形状（纹理增量要清掉，无头环境没人消费）。
pub(crate) fn frame_shapes(ctx: &egui::Context, app: &mut DecemApp) -> Vec<egui::Shape> {
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(980.0, 720.0),
        )),
        ..Default::default()
    };
    let mut output = ctx.run_ui(raw_input, |ui| {
        let _ = app.draw(ui);
    });
    output.textures_delta.clear();
    output
        .shapes
        .into_iter()
        .map(|clipped| clipped.shape)
        .collect()
}

pub(crate) fn any_shape(shapes: &[egui::Shape], predicate: &impl Fn(&egui::Shape) -> bool) -> bool {
    shapes.iter().any(|shape| match shape {
        egui::Shape::Vec(children) => any_shape(children, predicate),
        other => predicate(other),
    })
}

/// 有没有哪个矩形用这个颜色铺底。
pub(crate) fn has_filled_rect(shapes: &[egui::Shape], color: egui::Color32) -> bool {
    any_shape(
        shapes,
        &|shape| matches!(shape, egui::Shape::Rect(rect) if rect.fill == color),
    )
}

/// 有没有哪条线/圆/矩形用这个颜色描边（图标就是描边画出来的）。
pub(crate) fn has_stroked_shape(shapes: &[egui::Shape], color: egui::Color32) -> bool {
    any_shape(shapes, &|shape| match shape {
        egui::Shape::Rect(rect) => rect.stroke.color == color,
        egui::Shape::Circle(circle) => circle.stroke.color == color,
        egui::Shape::Path(path) => {
            matches!(path.stroke.color, egui::epaint::ColorMode::Solid(inner) if inner == color)
        }
        _ => false,
    })
}

#[test]
fn composer_buttons_use_the_requested_orange() {
    let store = temp_store("flow-orange");
    let ctx = egui::Context::default();
    let mut app = DecemApp::build(store, &ctx, LoadedFonts::default(), None).expect("构造应用失败");
    let orange = crate::theme::BUTTON_ORANGE;

    // 空闲：附件图标与发送图标都是橙色，发送键仍是暗底（保留「还不能发」的提示）
    let idle = frame_shapes(&ctx, &mut app);
    assert!(has_stroked_shape(&idle, orange), "空闲时两个图标就该是橙色");
    assert!(
        !has_filled_rect(&idle, orange),
        "输入为空时发送键不该是橙底"
    );

    // 有内容可发：发送键变橙底，图标转白
    app.input = "在吗".to_owned();
    let ready = frame_shapes(&ctx, &mut app);
    assert!(has_filled_rect(&ready, orange), "能发送时发送键应是橙底");
    assert!(
        has_stroked_shape(&ready, egui::Color32::WHITE),
        "橙底上的纸飞机应是白色"
    );
}

/// 这一帧画出来的所有文字（拿 galley 里的原文）。
pub(crate) fn painted_text(shapes: &[egui::Shape]) -> String {
    fn collect(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Vec(children) => {
                for child in children {
                    collect(child, out);
                }
            }
            egui::Shape::Text(text) => {
                out.push_str(&text.galley.job.text);
                out.push('\n');
            }
            _ => {}
        }
    }

    let mut out = String::new();
    for shape in shapes {
        collect(shape, &mut out);
    }
    out
}

#[test]
fn image_only_message_has_no_empty_placeholder() {
    let store = temp_store("flow-image-only");
    let ctx = egui::Context::default();
    let mut app = DecemApp::build(store, &ctx, LoadedFonts::default(), None).expect("构造应用失败");
    app.messages = vec![decem_core::Message {
        id: "image-only".to_owned(),
        role: MessageRole::User,
        content: String::new(),
        timestamp: 1_700_000_000_000,
        image_url: Some("https://example.invalid/foo.png".to_owned()),
        image_path: None,
        hint: None,
    }];

    // 只发了图片：图片就是全部内容，不该再写「(空消息)」
    let shapes = frame_shapes(&ctx, &mut app);
    assert!(
        !painted_text(&shapes).contains("(空消息)"),
        "只发图片时不该出现占位文字：{}",
        painted_text(&shapes)
    );

    // 对照：既没有文字也没有图片时才提示
    if let Some(message) = app.messages.first_mut() {
        message.image_url = None;
    }
    let shapes = frame_shapes(&ctx, &mut app);
    assert!(
        painted_text(&shapes).contains("(空消息)"),
        "空消息应该还是给个提示"
    );
}

/// 画面上找出含指定文字的文本块的位置。
pub(crate) fn text_rect(shapes: &[egui::Shape], needle: &str) -> Option<egui::Rect> {
    fn find(shape: &egui::Shape, needle: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Vec(children) => children.iter().find_map(|child| find(child, needle)),
            egui::Shape::Text(text) if text.galley.job.text.contains(needle) => {
                Some(egui::Rect::from_min_size(text.pos, text.galley.size()))
            }
            _ => None,
        }
    }

    shapes.iter().find_map(|shape| find(shape, needle))
}

#[test]
fn regenerate_label_takes_clicks_and_shows_no_text_cursor() {
    let store = temp_store("flow-regenerate");
    let ctx = egui::Context::default();
    let mut app = DecemApp::build(store, &ctx, LoadedFonts::default(), None).expect("构造应用失败");
    app.messages = vec![decem_core::Message {
        id: "char-1".to_owned(),
        role: MessageRole::Assistant,
        content: "嗷呜～".to_owned(),
        timestamp: 1_700_000_000_000,
        image_url: None,
        image_path: None,
        hint: None,
    }];

    // 先定位「重新生成」这块文字
    let shapes = frame_shapes(&ctx, &mut app);
    let rect = text_rect(&shapes, "重新生成").expect("Decem 消息下方应有「重新生成」");
    let center = rect.center();

    // 指针停在文字上：这是可点按钮，不该变成输入态光标（网页版是 button + user-select: none）
    let over = frame(&ctx, &mut app, &[egui::Event::PointerMoved(center)]);
    assert_ne!(
        over.0,
        egui::CursorIcon::Text,
        "按钮里的文字不该出现输入态光标"
    );

    // 按一下应当产生「重新生成」动作
    let (_, actions) = frame(
        &ctx,
        &mut app,
        &[
            egui::Event::PointerMoved(center),
            egui::Event::PointerButton {
                pos: center,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: center,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert!(
        actions.contains(&Action::Regenerate),
        "点「重新生成」应触发重新生成：{actions:?}"
    );

    // 手抖一点也算点击：文字不可选中，就不会被「拖选文字」抢走
    let nudged = center + egui::vec2(3.0, 1.0);
    let (_, actions) = frame(
        &ctx,
        &mut app,
        &[
            egui::Event::PointerMoved(center),
            egui::Event::PointerButton {
                pos: center,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerMoved(nudged),
            egui::Event::PointerButton {
                pos: nudged,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert!(
        actions.contains(&Action::Regenerate),
        "手抖几像素也该算点击：{actions:?}"
    );
}

/// 跑一帧，返回（光标形状，这一帧产生的动作）。
fn frame(
    ctx: &egui::Context,
    app: &mut DecemApp,
    events: &[egui::Event],
) -> (egui::CursorIcon, Vec<Action>) {
    frame_at(ctx, app, events, None)
}

/// 同上，但可以指定这一帧的时间（用来模拟「按住不放」这类操作）。
fn frame_at(
    ctx: &egui::Context,
    app: &mut DecemApp,
    events: &[egui::Event],
    time: Option<f64>,
) -> (egui::CursorIcon, Vec<Action>) {
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(980.0, 720.0),
        )),
        time,
        events: events.to_vec(),
        ..Default::default()
    };
    let mut actions = Vec::new();
    let mut output = ctx.run_ui(raw_input, |ui| {
        actions = app.draw(ui);
    });
    output.textures_delta.clear();
    (output.platform_output.cursor_icon, actions)
}

#[test]
fn model_dropdown_selection_works() {
    let store = temp_store("flow-dropdown");
    let ctx = egui::Context::default();
    let mut app = DecemApp::build(store, &ctx, LoadedFonts::default(), None).expect("构造应用失败");
    app.settings.model = "qwen3.8-flash".to_owned();
    app.show_settings = true;

    // 抽屉是个 Area：头一两帧在量尺寸，先跑两帧让它稳定
    let _ = frame_shapes(&ctx, &mut app);
    let shapes = frame_shapes(&ctx, &mut app);
    let trigger = text_rect(&shapes, "Qwen3.8-Flash")
        .unwrap_or_else(|| panic!("画面上的文字是：{}", painted_text(&shapes)));
    let (_, actions) = frame(&ctx, &mut app, &click_events(trigger.center()));
    assert!(actions.is_empty(), "点触发器只该开合菜单：{actions:?}");

    // 弹窗第一帧在量尺寸，跑两帧让它稳定
    let _ = frame_shapes(&ctx, &mut app);
    let shapes = frame_shapes(&ctx, &mut app);
    let item = text_rect(&shapes, "GLM-5.2").expect("菜单里应有 GLM-5.2");

    // 点在菜单项的文字上：应当选中它（这也是「有时点不中」那个问题的守卫）
    let (_, actions) = frame(&ctx, &mut app, &click_events(item.center()));
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, Action::SetModel(id) if id == "glm-5.2")),
        "点菜单项应切换模型：{actions:?}"
    );
}

/// 一次「按下 + 松开」的事件序列。
fn click_events(pos: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

#[test]
fn a_slow_click_on_a_menu_item_still_selects() {
    // 用户报的「有时选不中」：菜单项按住想一下再松开（>0.8 秒）、或者按下期间手抖几像素
    //（>6px），egui 默认就不把这一下当点击了（`input_state/mod.rs:1122`）。
    // 放宽阈值（`app::relax_click_thresholds`）之后仍要能选中。
    let store = temp_store("flow-slow-click");
    let ctx = egui::Context::default();
    let mut app = DecemApp::build(store, &ctx, LoadedFonts::default(), None).expect("构造应用失败");
    app.settings.model = "qwen3.8-flash".to_owned();
    app.show_settings = true;

    let _ = frame_shapes(&ctx, &mut app);
    let shapes = frame_shapes(&ctx, &mut app);
    let trigger = text_rect(&shapes, "Qwen3.8-Flash").expect("抽屉里应有模型触发器");
    let _ = frame(&ctx, &mut app, &click_events(trigger.center()));
    let _ = frame_shapes(&ctx, &mut app);
    let shapes = frame_shapes(&ctx, &mut app);
    let item = text_rect(&shapes, "GLM-5.2").expect("菜单里应有 GLM-5.2");
    let press_at = item.center();
    let release_at = press_at + egui::vec2(8.0, 2.0);

    // 按下（t=1.0）
    let _ = frame_at(
        &ctx,
        &mut app,
        &[
            egui::Event::PointerMoved(press_at),
            egui::Event::PointerButton {
                pos: press_at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        Some(1.0),
    );
    // 按住想了一下（1.2 秒），手还挪了几像素
    let _ = frame_at(
        &ctx,
        &mut app,
        &[egui::Event::PointerMoved(release_at)],
        Some(2.2),
    );
    // 松开
    let (_, actions) = frame_at(
        &ctx,
        &mut app,
        &[egui::Event::PointerButton {
            pos: release_at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        Some(2.2),
    );
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, Action::SetModel(id) if id == "glm-5.2")),
        "按住一会儿再松开也该算选中：{actions:?}"
    );
}

#[test]
fn clicking_the_drawers_empty_area_does_not_close_it() {
    // 抽屉里点空白处不能把面板关掉（网页版也是如此）。
    // 屏幕右下角一定在抽屉里（抽屉贴右、铺满整个高度），那里没有任何控件。
    let ctx = egui::Context::default();
    let mut app = DecemApp::build(
        temp_store("flow-drawer"),
        &ctx,
        LoadedFonts::default(),
        None,
    )
    .expect("构造应用失败");
    app.show_settings = true;

    let _ = frame_shapes(&ctx, &mut app);
    let _ = frame_shapes(&ctx, &mut app);

    let (_, actions) = frame(&ctx, &mut app, &click_events(egui::pos2(974.0, 714.0)));
    assert!(
        !actions.contains(&Action::ToggleSettings),
        "点抽屉里的空白处不该把面板关掉：{actions:?}"
    );

    // 反过来：点左边那片遮罩仍然要能关掉面板
    let (_, actions) = frame(&ctx, &mut app, &click_events(egui::pos2(20.0, 400.0)));
    assert!(
        actions.contains(&Action::ToggleSettings),
        "点遮罩应当关掉面板：{actions:?}"
    );
}

#[test]
fn pressing_the_mask_does_not_make_the_panel_unclickable() {
    // 用户报的「面板有时点不了」：egui 会把「被按下的 Area」提到同层最前面
    //（`containers/area.rs` 的 move_to_top）。遮罩和抽屉同层时，
    //「在遮罩上按下 → 拖进抽屉里松开」这一下远到不算点击（面板不会关），
    // 却已经把遮罩提到抽屉前面 —— 此后抽屉里怎么点都是点到遮罩，面板就废了。
    let ctx = egui::Context::default();
    let mut app = DecemApp::build(
        temp_store("flow-mask-press"),
        &ctx,
        LoadedFonts::default(),
        None,
    )
    .expect("构造应用失败");
    app.show_settings = true;

    let _ = frame_shapes(&ctx, &mut app);
    let shapes = frame_shapes(&ctx, &mut app);
    let upload_at = text_rect(&shapes, "上传头像")
        .expect("抽屉里应有「上传头像」")
        .center();

    // 在遮罩上按下……
    let (_, actions) = frame_at(
        &ctx,
        &mut app,
        &[
            egui::Event::PointerMoved(egui::pos2(100.0, 400.0)),
            egui::Event::PointerButton {
                pos: egui::pos2(100.0, 400.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        Some(1.0),
    );
    assert!(!actions.contains(&Action::ToggleSettings), "按下不该关面板");

    // ……拖进抽屉里再松开（移动很远，所以这一下不算点击）
    let _ = frame_at(
        &ctx,
        &mut app,
        &[egui::Event::PointerMoved(egui::pos2(700.0, 400.0))],
        Some(1.1),
    );
    let (_, actions) = frame_at(
        &ctx,
        &mut app,
        &[egui::Event::PointerButton {
            pos: egui::pos2(700.0, 400.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        Some(1.2),
    );
    assert!(
        !actions.contains(&Action::ToggleSettings),
        "这一下不算点击，不该关面板：{actions:?}"
    );

    // 现在点抽屉里的「上传头像」：必须还能点到
    let (_, actions) = frame(&ctx, &mut app, &click_events(upload_at));
    assert!(
        actions.contains(&Action::PickAvatar),
        "抽屉被遮罩盖住后点不动了：{actions:?}"
    );
}
