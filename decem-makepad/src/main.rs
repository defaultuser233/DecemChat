//! Decem Chat 的 Makepad 版 —— 阶段 1：聊天骨架。
//!
//! 这一阶段**不接网络**，用一个进程内的模拟上游把字块吐出来，先把 Makepad 本身证伪：
//! 消息列表、输入框（含中文输入法）、回车发送、流式打字机、长消息换行与滚动。
//!
//! 结构照 `makepad/examples/aichat` 的 proven 模式：
//! 全局 `RwLock` 状态 + 工作线程 → channel → `SignalToUI` 唤醒 UI → `handle_event` 里轮询。

pub use makepad_widgets;

use makepad_widgets::desktop_style::{self, DesktopStyle, StyleSheet};
use makepad_widgets::makepad_platform::thread::SignalToUI;
use makepad_widgets::*;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, RwLock};
use std::time::Duration;

app_main!(App);

/// 角色。项目命名规范：Decem 一侧叫 char（不用 bot / assistant）。
#[derive(Clone, Copy, PartialEq)]
enum Role {
    User,
    Char,
}

struct Msg {
    role: Role,
    text: String,
}

/// 全局聊天状态（照 aichat 的做法：`ChatList` 要在 `draw_walk` 里读它）。
struct Chat {
    messages: Vec<Msg>,
    streaming: String,
    is_streaming: bool,
}

static CHAT: RwLock<Chat> = RwLock::new(Chat {
    messages: Vec::new(),
    streaming: String::new(),
    is_streaming: false,
});

const WELCOME: &str = "🦊 狐狸来啦～ (≧▽≦)/～ ❤️ 你终于来找我玩啦！嗷呜～";

/// 模拟上游的回复：刻意包含中文、全角标点、emoji、代码块与表格，
/// 一次把阶段 1 的几个关注点都过一遍。
fn simulated_reply() -> String {
    "收到啦～这是**模拟上游**吐出来的回复，用来验证流式打字机。\n\n\
     中文与全角标点：你好，世界！？～（）【】\n\n\
     emoji 测试：🦊 ❤️ 🎉\n\n\
     ```rust\nfn main() { println!(hi); }\n```\n\n\
     | 元素 | 符号 | 备注 |\n| --- | --- | --- |\n| 中文 | **粗体** | 全角：？！～ |\n| emoji | 🦊 | 彩色 |\n\n\
     长文本换行：这是一段刻意写长的中文，用来观察在列表里它会不会正常换行、行距是否舒服。"
        .to_owned()
}

// ---------------------------------------------------------------- 模拟上游

enum WorkerEvent {
    Delta(String),
    Done,
}

struct Worker {
    rx: mpsc::Receiver<WorkerEvent>,
    cancel: Arc<AtomicBool>,
}

impl Worker {
    /// 起一个线程，按 3 个字符一批、每批 28ms 的节奏吐字块（模拟 SSE 的字块到达）。
    fn spawn_simulated(reply: String) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();

        std::thread::spawn(move || {
            let mut buf = String::new();
            for ch in reply.chars() {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                buf.push(ch);
                if buf.chars().count() >= 3 {
                    if tx
                        .send(WorkerEvent::Delta(std::mem::take(&mut buf)))
                        .is_err()
                    {
                        return;
                    }
                    SignalToUI::set_ui_signal();
                    std::thread::sleep(Duration::from_millis(28));
                }
            }
            if !buf.is_empty() && !flag.load(Ordering::Relaxed) {
                let _ = tx.send(WorkerEvent::Delta(buf));
            }
            let _ = tx.send(WorkerEvent::Done);
            SignalToUI::set_ui_signal();
        });

        Self { rx, cancel }
    }

    fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn poll(&self) -> Vec<WorkerEvent> {
        self.rx.try_iter().collect()
    }
}

// ---------------------------------------------------------------- 气泡定宽

/// `ChatList` 画每一项之前把它放进来，`Bubble` 在自己 `draw_walk` 里取。
/// 绘制是同一线程、逐项同步进行的，所以一个「当前项」槽位就够。
static BUBBLE_BODY: RwLock<Option<(String, bool)>> = RwLock::new(None);

/// 正文里有没有块级元素。代码块和表格的宽度不由正文字号决定，量文字没有意义，
/// 这类消息直接用满宽。
fn has_block_content(text: &str) -> bool {
    text.contains("```") || text.contains("\n|")
}

/// 量宽容器：先按上限把正文量一遍，再用**量出来的实际宽度**给气泡定宽，
/// 于是短消息贴着文字、长消息到上限换行。
///
/// 为什么非自己量不可：`Markdown`/`TextFlow` 的换行宽度取的是外层 turtle 的内宽
/// （`widgets/families/rich_text/src/text_flow.rs:2554-2559`），而 `Fit` 的 turtle
/// 没有确定宽度（NaN）就不换行 —— 长文本会直接溢出被裁。Makepad 自带的
/// `ChatBubble` 用的是同一个办法（`widgets/families/extras/src/chat.rs:903-924`）：
/// 按上限把文字排一遍，拿用到的宽度决定气泡多宽。这里照它写，内容仍交给
/// `Markdown` 画（官方注释也说了：需要标题和链接的消息是「气泡形容器里的
/// Markdown」，不是更大的气泡 —— chat.rs:31-34）。
#[derive(Script, ScriptHook, Widget)]
pub struct Bubble {
    #[deref]
    view: View,
    /// 只用来量正文宽度。字体必须与 Markdown 正文一致
    #[live]
    draw_text: DrawText,
    /// 气泡最宽多少
    #[live(560.0)]
    cap_width: f64,
    /// 气泡的左右内边距之和（与模板里 RoundedView 的 padding 对应）
    #[live(28.0)]
    pad_x: f64,
}

impl Widget for Bubble {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let (body, blocky) = BUBBLE_BODY
            .read()
            .map(|slot| slot.clone())
            .unwrap_or(None)
            .unwrap_or_default();

        // 此刻最上层还是外层行（Fill），所以这个内宽是确定的
        let row_w = cx.turtle().inner_rect().size.x;
        let row_w = if row_w.is_finite() {
            row_w
        } else {
            self.cap_width
        };
        let cap = row_w.clamp(60.0, self.cap_width);
        let inner_cap = (cap - self.pad_x).max(24.0);

        let width = if blocky || body.is_empty() {
            cap
        } else {
            let scale = (self.draw_text.font_scale as f64).max(0.0001);
            let laid = self.draw_text.layout(
                cx,
                0.0,
                0.0,
                Some((inner_cap / scale) as f32),
                true,
                Align::default(),
                &body,
            );
            let used = laid.size_in_lpxs.width as f64 * scale;
            // 余量：这只测量实例与 Markdown 的渲染实例不是同一个，
            // emoji 与字距会有几像素出入，不够余量就会提前折行
            (used + self.pad_x + 8.0).clamp(60.0, cap)
        };

        // 给子部件一个确定宽度：Markdown 的换行宽度就是从这里来的。
        // 子层的 step 自己走完再交回 done —— 这一层没有需要上层驱动的部件
        // （PortalList 之类），照 Widget::draw_all 的循环语义来
        let child = Walk {
            width: Size::Fixed(width),
            height: Size::fit(),
            ..walk
        };
        let mut step = self.view.draw_walk(cx, scope, child);
        while step.is_step() {
            step = self.view.draw_walk(cx, scope, child);
        }
        DrawStep::done()
    }
}

// ---------------------------------------------------------------- 消息列表

/// 包一层 PortalList：列表项的文字必须由 Rust 侧按数据逐项写入。
#[derive(Script, ScriptHook, Widget)]
pub struct ChatList {
    #[deref]
    view: View,
    /// 正在打字的那一项（只在它刚开始时重置动画）。
    #[rust]
    animating: Option<usize>,
}

impl Widget for ChatList {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let chat = CHAT.read().unwrap();

        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = item.as_portal_list().borrow_mut() {
                let count = chat.messages.len();
                let items_len = count + chat.is_streaming as usize;
                list.set_item_range(cx, 0, items_len);

                while let Some(item_id) = list.next_visible_item(cx) {
                    // 正在流式的那一条：同一个 item 反复重绘，交给 Markdown 的流式动画逐字显形
                    if chat.is_streaming && item_id == count {
                        let just_started = self.animating != Some(item_id);
                        if just_started {
                            self.animating = Some(item_id);
                        }
                        let (item_widget, _) = list.item_with_existed(cx, item_id, id!(Char));
                        let text = if chat.streaming.is_empty() {
                            "..."
                        } else {
                            &chat.streaming
                        };
                        let mut markdown = item_widget.markdown(cx, ids!(selectable));
                        markdown.set_text(cx, text);
                        if just_started {
                            markdown.reset_all_streaming_animations();
                        } else {
                            markdown.start_streaming_animation();
                        }
                        *BUBBLE_BODY.write().unwrap() =
                            Some((text.to_owned(), has_block_content(text)));
                        item_widget.draw_all_unscoped(cx);
                        *BUBBLE_BODY.write().unwrap() = None;
                        continue;
                    }

                    if let Some(msg) = chat.messages.get(item_id) {
                        let was_animating = self.animating == Some(item_id);
                        let template = match msg.role {
                            Role::User => id!(User),
                            Role::Char => id!(Char),
                        };
                        let item_widget = list.item(cx, item_id, template);
                        let mut markdown = item_widget.markdown(cx, ids!(selectable));
                        markdown.set_text(cx, &msg.text);
                        if was_animating {
                            markdown.stop_streaming_animation();
                        }
                        *BUBBLE_BODY.write().unwrap() =
                            Some((msg.text.clone(), has_block_content(&msg.text)));
                        item_widget.draw_all_unscoped(cx);
                        *BUBBLE_BODY.write().unwrap() = None;
                        if was_animating && markdown.is_streaming_animation_done() {
                            self.animating = None;
                        }
                    }
                }
            }
        }
        DrawStep::done()
    }
}

// ---------------------------------------------------------------- 应用

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    worker: Option<Worker>,
}

impl App {
    fn send(&mut self, cx: &mut Cx) {
        let input = self.ui.text_input(cx, ids!(input));
        let text = input.text().trim().to_owned();
        if text.is_empty() {
            return;
        }
        input.set_text(cx, "");

        let items_len = {
            let mut chat = CHAT.write().unwrap();
            chat.messages.push(Msg {
                role: Role::User,
                text,
            });
            chat.streaming.clear();
            chat.is_streaming = true;
            chat.messages.len() + 1
        };

        self.worker = Some(Worker::spawn_simulated(simulated_reply()));

        let list = self
            .ui
            .widget(cx, ids!(chat_list))
            .portal_list(cx, ids!(list));
        list.set_tail_range(true);
        list.set_first_id_and_scroll(items_len.saturating_sub(1), 0.0);
        cx.redraw_all();
    }

    /// Esc 中断：把已经流出来的部分定格成一条消息。
    fn cancel(&mut self, cx: &mut Cx) {
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        self.finish_stream();
        self.worker = None;
        cx.redraw_all();
    }

    fn finish_stream(&self) {
        let mut chat = CHAT.write().unwrap();
        let text = std::mem::take(&mut chat.streaming);
        if !text.is_empty() {
            chat.messages.push(Msg {
                role: Role::Char,
                text,
            });
        }
        chat.is_streaming = false;
    }

    fn poll_worker(&mut self, cx: &mut Cx) {
        let events = match &self.worker {
            Some(worker) => worker.poll(),
            None => return,
        };
        let had_events = !events.is_empty();
        let mut done = false;
        for event in events {
            match event {
                WorkerEvent::Delta(delta) => {
                    CHAT.write().unwrap().streaming.push_str(&delta);
                }
                WorkerEvent::Done => done = true,
            }
        }
        if done {
            self.finish_stream();
            self.worker = None;
        }
        if had_events {
            cx.redraw_all();
        }
    }
}

impl MatchEvent for App {
    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.glass_button(cx, ids!(send_button)).clicked(actions) {
            self.send(cx);
        }
        if self
            .ui
            .text_input(cx, ids!(input))
            .returned(actions)
            .is_some()
        {
            self.send(cx);
        }
        if self.ui.text_input(cx, ids!(input)).escaped(actions) {
            self.cancel(cx);
        }
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        // 近黑 + 橙的风格（Makepad 自带），与 Decem 的调性接近
        desktop_style::install(vm, StyleSheet::load(DesktopStyle::BlackOrange));
        self::script_mod(vm)
    }

    fn after_new_from_script(_vm: &mut ScriptVm, app: &mut Self) {
        let mut chat = CHAT.write().unwrap();
        chat.messages.push(Msg {
            role: Role::Char,
            text: WELCOME.to_owned(),
        });
        // 压测：DECEM_STRESS=1 时塞几百条，观察长列表滚动
        if std::env::var("DECEM_STRESS").is_ok() {
            for i in 0..300 {
                let role = if i % 2 == 0 { Role::User } else { Role::Char };
                chat.messages.push(Msg {
                    role,
                    text: format!(
                        "压测第 {i} 条：中文、全角标点？！～、emoji 🦊，以及一段用来观察换行的稍长文本。"
                    ),
                });
            }
        }
        drop(chat);
        let _ = app;
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
        self.poll_worker(cx);
    }
}

script_mod! {
    use mod.prelude.widgets.*
    // 代码块用 makepad-widgets 自带的 CodeBlock：它在 Fit 宽度下会量最长行。
    // 别用 CodeView —— 那属于独立的 makepad-code-editor crate，不引它模板会静默失效。
    use mod.widgets.CodeBlock

    // 气泡定宽容器：自己量正文宽度，短消息贴文字、长消息到上限换行。
    // 见 docs/makepad-migration.md §6.2
    let Bubble = #(Bubble::register_widget(vm)) {
        width: Fit
        height: Fit
        // 量宽用的字体必须与 Markdown 正文一致（markdown.rs:101-102）
        draw_text +: { text_style: theme.font_regular{font_size: theme.font_size_p} }
        cap_width: 560.0
        pad_x: 28.0
    }

    let ChatList = #(ChatList::register_widget(vm)) {
        width: Fill
        height: Fill

        list := PortalList {
            width: Fill
            height: Fill
            flow: Down
            auto_tail: true
            smooth_tail: true
            reuse_items: false

            // Decem 一侧：左对齐、深色气泡
            Char := View {
                width: Fill
                height: Fit
                align: Align{x: 0.0}
                padding: Inset{top: 6 bottom: 6 left: 4 right: 64}

                bubble := Bubble {
                    RoundedView {
                        width: Fill
                        height: Fit
                        show_bg: true
                        draw_bg.color: #x24242c
                        draw_bg.border_radius: uniform(16.)
                        padding: Inset{left: 14 top: 10 right: 14 bottom: 10}

                        selectable := Markdown {
                            // 这里必须是 Fill：Bubble 已经把确定宽度给了外层，
                            // Fill 才能拿到它并按它换行
                            width: Fill
                            height: Fit
                            body: ""
                            use_code_block_widget: true
                            use_math_widget: true
                            code_block := View {
                                width: Fill
                                height: Fit
                                code_view := CodeBlock {
                                    width: Fill
                                    height: Fit
                                    show_line_numbers: false
                                }
                            }
                        }
                    }
                }
            }

            // 用户一侧：右对齐、橙色气泡（与网页版同一个橙 #E95620）
            User := View {
                width: Fill
                height: Fit
                align: Align{x: 1.0}
                padding: Inset{top: 6 bottom: 6 left: 64 right: 4}

                bubble := Bubble {
                    RoundedView {
                        width: Fill
                        height: Fit
                        show_bg: true
                        draw_bg.color: #xE95620
                        draw_bg.border_radius: uniform(16.)
                        padding: Inset{left: 14 top: 10 right: 14 bottom: 10}

                        selectable := Markdown {
                            width: Fill
                            height: Fit
                            body: ""
                        }
                    }
                }
            }
        }
    }

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Decem Chat"
                window.inner_size: vec2(980, 760)
                body +: {
                    View{
                        width: Fill
                        height: Fill
                        flow: Down
                        spacing: 10.
                        padding: Inset{left: 14 top: 10 right: 14 bottom: 12}
                        show_bg: true
                        draw_bg.color: theme.color_bg_app

                        chat_list := ChatList {}

                        View{
                            width: Fill
                            height: Fit
                            flow: Right
                            spacing: 8.
                            align: Align{y: 1.0}

                            input := glass.TextInput {
                                width: Fill
                                height: 44
                                empty_text: "发个消息聊聊呗～（Enter 发送，Esc 中断）"
                            }

                            send_button := glass.GlassButtonProminent {
                                text: "发送"
                                width: 84
                                height: 44
                            }
                        }
                    }
                }
            }
        }
    }
}
