# Makepad 迁移：状态、计划与作业手册

> 更新：2026-10-16　关联文档：`docs/makepad-probe.md`（探针实测记录与候选筛选）
> 这份文档的目的：**换一个会话、隔几周回来，也能立刻接着干**。

## 0. 一句话状态

方向已定：**用 Makepad（纯 Rust 自绘）重做 Decem Chat**，分四阶段、每阶段有验收门槛。
当前进度：**阶段 1（骨架）与阶段 2（真实网络）都已完成并实测通过**（`decem-makepad/`）。
骨架能跑、中文输入法能上屏、流式打字机正常、300 条长列表滚动跟手；
现在发出去的是真请求，`decem-core` 的 SSE 解析器接 `blocking_http` 的阻塞流，
Esc 能中断真实流。证据见 §3 与 §9。
**下一道门是阶段 3：功能对齐长尾**（图片消息、重新生成、清空、模型切换、设置、持久化……）。
现有实现（网页版 / Tauri 版 / egui 版）**全部保留、一行未动**，继续是主力。

历史坑（都结清了，别再重踩）：
- 代码块/表格渲染成空白 → 根因是 Markdown 的换行宽度只能由外层给确定值，见 §6.1/§6.2；
  现在的解法是 `Bubble` 部件自己量宽（§6.3）
- 气泡「贴文字」和「正常换行」一度被判为不能兼得 —— **那个结论是错的**，已解决，见 §6.3
- 滚动"没反应"：多次滚轮挤在紧循环里发、或自动化滚轮投递丢失，不是 app 的问题。判据是
  `debug_scroll_state_line()` 里的 `first_scroll`/`tail_range`，见 §3

## 1. 资产与坐标（别丢）

| 东西 | 位置 | 说明 |
| --- | --- | --- |
| 主仓库 | `D:\Documents\DecemChat` | `src/**`（网页版 React）、`src-tauri/`（Tauri 壳）、`crates/`（旧 egui 版 + decem-core）、**`decem-makepad/`（新）**、`docs/` |
| Makepad 克隆 | `D:\Documents\MakepadProbe\makepad`（dev @`71c9ddcb`） | 离线查源码/示例用；正式依赖走 git，不依赖这份克隆 |
| 探针源码 | `D:\Documents\MakepadProbe\decem-probe`（path 依赖）／`decem-probe-git`（git 依赖） | UI 组件与配色的可运行参照 |
| 探针 APK | `D:\Documents\MakepadProbe\decem-probe-android.apk`（40 MB） | 纯 UI 探针，**真机渲染未验** |
| Tauri 版 APK | `D:\Documents\DecemChat\decem-chat-android-release.apk`（18 MB，V2 签名 `CN=Decem Chat`） | 手机上的现役版本；含 edge-to-edge inset 修复 |
| Android SDK | `D:\Android\Sdk` | 两套工具链共用：Tauri 用 platform 37 / build-tools 37 / **NDK 29.0.13846066**；Makepad 自带 **NDK r28b + JDK 17** |
| release keystore | `C:\Users\MEC\.decem-chat\decem-release.jks`（密码见同目录 `password.txt`） | Android release 签名；**丢了就只能卸载重装** |
| 接口地址 | `https://api.valedecem.top:8443/api/chat` | nginx → `127.0.0.1:3000`；Key 在服务器上，客户端不用填 |

## 2. 分阶段计划

### 阶段 1：聊天骨架（✅ 已通过验收，记录见 §3）
- **目标**：消息列表 + 输入 + 发送 + 流式打字机，数据来自**进程内模拟上游**（先不接网络）
- **为什么先不接网络**：这一步证伪的是 Makepad 本身；网络是下一道独立的门，混在一起失败时无法归因
- **验收（要人亲自跑）**：① 中文输入法能敲字 ② 回车/点按钮发送 ③ 回复逐字出现 ④ 长文本换行、列表滚动 ⑤ 全角标点与 emoji 正常
- ⚠️ **第 ① 条不过就停在这里**（中文聊天 App 没法用），届时如实报告而不是硬撑

### 阶段 2：接真实网络（✅ 已实现并实测，记录见 §9）
工作线程里用 `makepad_platform::makepad_network::blocking_http`（**HTTPS、只走一跳**，可边收边写给 writer）
→ 复用 `decem-core` 的 SSE 解析器与请求编排 → channel + `SignalToUI` 推给 UI → 交给 Markdown 的流式动画。


### 阶段 3：功能对齐（长尾）
头像与图片消息（base64 解码 + Makepad 的 `Image`）、重新生成、清空记录（二次确认）、
模型切换面板与「视觉理解」标注、设置（API Key / 接口地址）、本地持久化、主题、Toast、欢迎语、打字速度……

### 阶段 4：全平台出包
Android 已有完整流程（见 §4）；Windows 桌面打包**待核实**（官方无工具，社区 `cargo-packager`）；
macOS/Linux 无机器可验（可 CI 编译）；iOS 需 Mac + Xcode。

## 3. 阶段 1 验收结果（已通过）

代码：`decem-makepad/src/main.rs`（约 300 行）

- `PortalList` 消息列表（左右两种气泡模板，`Char` 左/深色，`User` 右/橙 `#xE95620`）
- **`Bubble` 部件**：自己量正文宽度再给 Markdown 定宽，于是气泡贴着文字、长文本到上限换行（§6.3）
- `glass.TextInput` + `glass.GlassButtonProminent`，回车发送、Esc 中断
- **模拟上游**：工作线程按 3 字符/28ms 吐块 → `SignalToUI` → UI 轮询 → Markdown 流式动画逐字显形
- `DECEM_STRESS=1` 启动时种 300 条消息，用于压测滚动

实测（2026-10-16，Windows，release 构建，截图见会话记录）：

| 验收项 | 结论 | 怎么验的 |
| --- | --- | --- |
| 中文输入法（IME）合成 | ✅ | Ctrl+Space 切中文 → 敲 `nihao` → 输入框显示合成串 `ni'hao` → 空格上屏得到「你好」→ 回车发出 |
| 回车发送 / 点「发送」按钮 | ✅ | 两条路径都发出去了，输入框自动清空 |
| 流式打字机 | ✅ | 回复逐块出现；Esc 能中断并把已流出的部分定格成一条消息 |
| 中文/全角标点/emoji | ✅ | 「你好，世界！？～（）【】」与 🦊❤️🎉 全部正常渲染（彩色 emoji） |
| 代码块 + 表格 | ✅ | 代码块用 `CodeBlock`（独立底色 + Copy 按钮），表格列对齐，见 §6.1 |
| 气泡贴文字 + 窄窗口不溢出 | ✅ | 默认 980 宽时短消息紧凑、长消息到上限换行；缩到 664px 全部重排无裁切，见 §6.3 |
| 300 条长列表 | ✅ | 300 条全部渲染；滚轮上下滚动都跟手，滚动条正常 |

⚠️ **滚动那一条踩过假警报**：一次性把 6 次 `scroll` 挤在紧循环里发出去，列表纹丝不动，
看起来像「列表不能滚」。改成**每次滚动之间留 ~0.7–0.9s**、逐次发，就完全正常。
用 `PortalList::debug_scroll_state_line()` + `user_scroll_travel()` 拿到确凿证据：

```
before  first_id=288 first_scroll=-47.00  tail_range=true   travel=0
after   first_id=288 first_scroll=453.00  tail_range=false  travel=500   detect_tail=true
```

即 500px 的用户滚动被完整应用了。**结论：不是 app 的问题，是自动化测试的调用节奏问题**，
真人用鼠标滚轮不受影响。下次做类似自动化时，滚动之间务必留间隔。

```bash
cd D:\Documents\DecemChat\decem-makepad
cargo run --release                    # 普通跑
DECEM_STRESS=1 cargo run --release     # 300 条压测
```

## 4. Android 出包（Makepad 版）

```bash
# 一次性：编工具 + 装工具链（会往 D:/Android/Sdk 里加自己那套组件）
cd <makepad 克隆> && cargo install --path=./tools/cargo_makepad
cargo makepad android --abi=aarch64 --sdk-path='D:/Android/Sdk' \
  --package-name=top.valedecem.<pkg> --app-label='…' install-toolchain

# 每次出包（-p 必填）
cargo makepad android --abi=aarch64 --sdk-path='D:/Android/Sdk' \
  --package-name=top.valedecem.<pkg> --app-label='…' build -p <crate> --release
```
产物在 `<crate>/target/android/makepad-android-apk/<pkg>/apk/*.apk`，默认 **debug keystore 签名**（可侧载）。
正式版要换成我们的 release keystore（阶段 4）。

## 5. Makepad API 速查（从 `examples/aichat` 抄下来的七个模式）

| 要做什么 | 怎么写 | 出处 |
| --- | --- | --- |
| 注册 Rust 自定义 widget | `let ChatList = #(ChatList::register_widget(vm)) { ... }` | `examples/aichat/src/main.rs:26` |
| 滚动列表 | `PortalList{ auto_tail: true smooth_tail: true reuse_items: false }` + 模板 `User := View{...}` | 同上 `:30-140` |
| Rust 侧逐项画 | `set_item_range` / `next_visible_item` / `item(cx,id,template)` / `item_with_existed` / `draw_all_unscoped`，结尾 `DrawStep::done()` | 同上 `:418-472` |
| **流式打字机** | `markdown.reset_all_streaming_animations()` → `start_streaming_animation()` → `is_streaming_animation_done()` | 同上 `:444-465` |
| 回车 / 按钮 | `.returned(actions).is_some()` / `.clicked(actions)` / `.escaped(actions)` | 同上 `:774-794` |
| 线程 → UI | 线程 + `mpsc` + `SignalToUI::set_ui_signal()`；UI 侧在 `handle_event` 里 `poll()` | 同上 `:579-586`、`:846+` |
| 主题 | `desktop_style::install(vm, StyleSheet::load(DesktopStyle::BlackOrange))` | `widgets/core/src/desktop_style.rs:225` |
| **列表状态诊断**（排查滚动/尾部跟随的神器） | `list.debug_scroll_state_line()`（一行状态串）+ `list.user_scroll_travel()`（用户滚动累计位移，px） | `widgets/core/src/portal_list.rs:4448`、`:4440` |

> 读文本 `input.text()`、写文本 `input.set_text(cx, ...)`（`aichat:679-716`）。
> `debug_scroll_state_line()` 输出里几个关键字段：`tail_range`（是否正在跟随末尾）、
> `at_end`、`first_id`/`first_scroll`（视口位置）、`travel`（用户滚动累计量）。

## 6. 必读的三个坑（完整版见 `docs/makepad-probe.md` §5）

1. **不装暗色风格** → 行内代码在高亮块上几乎看不见（浅色主题压自绘深底）
2. **Markdown 的换行宽度只能由外层给**（`Fill` 或 `Fixed`，不能是 `Fit`）—— 阶段 1 最大的坑，
   完整机制与解法见 §6.2 / §6.3。现在由 `Bubble` 部件自己量宽后给 `Fixed`
3. **气泡底色别与行内代码底色用同一个主题变量**（`theme.color_bg_highlight` 会糊在一起）

### 6.1 代码块：`code_block` 模板是必填的

**Makepad 没有默认的 `code_block` 模板**（`markdown.rs` 与 `text_flow.rs` 里都没有）。
`use_code_block_widget: true` 会去找 `id!(code_block)` 这个模板，里面**必须有一个叫 `code_view`
的子部件**——`markdown.rs:455-458` 是这样把内容塞进去的：

```rust
tf.item_with(cx, entry_id, id!(code_block), |cx, item, _tf| {
    item.widget(cx, ids!(code_view)).set_text(cx, cbs);   // ← 名字写死是 code_view
    item.draw_all_unscoped(cx);
});
```

模板缺失或写错，**不会报编译错误**，只在运行时 stdout 里留一行 `[E]`，代码块表现为
"占了一段高度但什么都没有"。填内容有两种部件：

| 用哪个 | 归属 | 代价 | 长得像 |
| --- | --- | --- | --- |
| **`CodeBlock`（当前采用）** | `makepad-widgets`（`widgets/core/src/code_block.rs:58`） | 无，不用加依赖 | 深色底 + Copy 按钮，无语法高亮 |
| `CodeView` | 独立 crate `makepad-code-editor` | 要加依赖 + 在 `script_mod` 里调 `makepad_code_editor::script_mod(vm)`（`code_editor/src/lib.rs:33`） | 真编辑器，可编辑 |
| `use_code_block_widget: false` | — | 无 | 纯等宽文本，连底色都没有（最丑的兜底） |

`CodeBlock` 还有个关键优势：它在 `Fit` 宽度下会**主动去量最长行**（`code_block.rs:729-737`），
所以是唯一能在贴文字的气泡里正常工作的代码块。

### 6.2 宽度模型：为什么布局引擎不会自动给你「贴文字 + 正常换行」

这一节是阶段 1 踩得最久的地方，三个事实叠在一起（**注意：这是"为什么不能靠写 CSS 式的
width 声明解决"，不是"无解"——解法见 §6.3**）：

**① `Fill` 不是「100% 父宽」，是 flex 式的增长。**

```rust
// draw/src/turtle.rs:311-325
Fill { weight: f64, basis: FitBound /*默认 Abs(0.0)*/, shrink: f64, min, max }
```

`basis` 默认 0，靠 `weight` 分父容器的**剩余宽度**。父容器自己是 `Fit`（还没算出宽度）时
没有"剩余"可分 → 子元素宽度 = basis = **0**。这就是 `Fit` 气泡里代码块/表格变空白的机制。
（试错记录：只把内层 `Markdown` 改成 `Fill`、气泡仍是 `Fit`，整条气泡会塌成约 40px 宽的竖条、
逐字换行，把"宽度归零"暴露得更彻底。）

**② Markdown 的 `Fit` 量的是最长行的自然宽度，不换行。**

`width: Fit` 的 Markdown 不会在容器里折行，而是把最长那行完整撑开。后果：
- 长段落 → 气泡比窗口还宽，**内容被裁掉**（缩窄窗口实测：文字不重排，直接被切）
- 手机上这是致命的（屏幕才 400 逻辑像素宽）

**③ 表格的单元格宽度 = 行的 `inner_width` ÷ 列数。**

```rust
// widgets/families/rich_text/src/text_flow.rs:2184-2189
let cell_width = if self.table_num_columns > 0 {
    cx.turtle().inner_width() / self.table_num_columns as f64
} else { 100.0 };
```

所以**行必须有确定宽度**，否则整张表只剩高度。

### 6.3 解法：自己量宽，再把确定宽度交给 Markdown（已实装）

上面三条看起来互相矛盾，但它们其实指向同一个缺口：**换行宽度必须由外层给一个确定值**。
Makepad 的布局引擎不会替你算（没有 CSS 的 intrinsic sizing），**但你自己可以算**——
而且官方自带部件就是这么干的。

参考实现在 `ChatBubble`（`widgets/families/extras/src/chat.rs:903-924`）：

```rust
let scale = (self.draw_text.font_scale as f64).max(0.0001);
let body = self.draw_text.layout(
    cx, 0.0, 0.0, Some((inner_cap / scale) as f32), true, Align::default(), &self.text,
);
let body_size = dvec2(laid.size_in_lpxs.width as f64 * scale + TEXT_SLACK, ...);
```

即：**按上限把文字排一遍，拿"实际用到的宽度"当气泡宽度**。`DrawText::layout(...)` 是公开 API
（`draw/src/shader/draw_text.rs:2835`），返回的 `size_in_lpxs` 就是折行后的真实宽度 —— 短文本量出
自然宽度，超上限就被上限截住。

于是 `decem-makepad` 里加了一个 `Bubble` 部件（`src/main.rs`），流程是：

1. 取外层行的内宽（行是 `Fill`，所以确定）
2. `cap = min(行内宽, cap_width)`，再用 `DrawText::layout(..., Some(cap - pad_x), wrap=true, ...)`
   量出正文实际宽度
3. 给子部件一个 `Walk{ width: Size::Fixed(量出的宽度) }` → **Markdown 拿到确定宽度**，
   于是正常折行、代码块与表格也都正常（表格的行宽自然跟着确定下来）

这样三件事同时成立：

| 想要 | 结果 |
| --- | --- |
| 短消息贴着文字 | ✅ 按量出的宽度收窄 |
| 长文本到上限换行 / 窄窗口不溢出 | ✅ 换行宽度就是 `cap`，随窗口变 |
| 代码块 | ✅ 用 `CodeBlock`（见 §6.1） |
| 表格 | ✅ 行宽由气泡宽度决定，列自动对齐 |

两个必须记住的实现细节：

- **量宽用的字体必须与 Markdown 正文一致**，否则宽度系统性偏大/偏小。
  Markdown 正文是 `theme.font_regular{font_size: theme.font_size_p}`（`markdown.rs:101-102`），
  `Bubble` 的 `draw_text` 照抄。
- **要留几像素余量**。测量实例与 Markdown 的渲染实例不是同一个，emoji 与字距会有出入；
  不留余量会在边界上提前折行（实测：欢迎语多折了一行，加 8px 后正常）。
  ChatBubble 用 `TEXT_SLACK = 2.0`（`chat.rs:701`），它量的是自己那个字体实例所以够；
  跨实例要留大一点。

实测（Windows，release）：默认 980 宽时欢迎语一行、短消息是紧凑气泡；缩到 664px（约 443 逻辑像素）
后**全部自动重排、无溢出无裁切**，代码块与表格跟着变窄、列仍对齐；300 条压测下每条都贴文字，
滚动正常（`first_scroll` 变化 900px、`tail_range` 翻转，见 §3 的验证方法）。

## 7. 未决 / 只有人能验的

| 项 | 谁 |
| --- | --- |
| ~~中文输入法（IME）能否在 Makepad 输入框里正常合成~~ → **已验通**（§3） | ✅ 2026-10-16 |
| 气泡「贴文字」还是「通栏」 | ✅ 已解决（§6.3 的 `Bubble` 自己量宽） |
| Windows 上原生 TLS 是否撞证书吊销检查（兜底：换 rustls 客户端） | ✅ 没撞，不需要兜底（§9） |
| Makepad 桌面打包工具是否存在/好用 | 阶段 4 核实 |
| iOS（需 Mac）、macOS/Linux（无机器） | 待条件 |
| 旧实现处置：Makepad 版可用后，Tauri 版 / egui 版删不删 | 你 |
| 视觉目标：是否要像素级贴近现在的网页版（工时差别很大） | 你 |

## 8. 约定（别违反）

- Decem 一侧统一叫 **char**，不写 bot / assistant（`MessageRole::Assistant` 是接口协议，保留）
- 网页版 `src/**` 只作对齐依据，**不为 Makepad 版改动它**
- Makepad 版是独立 crate（自带 `[workspace]`，根 `Cargo.toml` 里已 `exclude`），不拖累现有 CI
- `AGENTS.md` 只读，不改

## 9. 阶段 2 实现记录（真实网络，2026-10-16）

### 9.1 通路

```
点击发送 → 组请求体（decem-core::ChatRequest::new：自带 system 提示词 + 最近 10 条）
        → 工作线程：blocking_http::Request::post(API_URL).json_body(..)
                      .limits(..).cancel_token(..)
        → request_to_writer_no_redirect(req, &mut SseSink)
        → SseSink::write()：decoder.push(buf) → parse_sse_line → channel + SignalToUI
        → UI 线程 handle_event 里 poll_worker → Markdown 流式动画
```

代码都在 `decem-makepad/src/main.rs`：`SseSink` / `upstream_error` / `Worker::spawn_chat` / `App::poll_worker`。

**为什么用 `request_to_writer_no_redirect`**：它把解码后的响应体**按到达顺序**边收边写进
`std::io::Write`，那个 writer 看到的就是网络的真实分包边界；跨包的半个事件、半个多字节字符
由 `SseDecoder` 兜住（它有单元测试：分包、半个汉字、CRLF、注释行、`[DONE]`）。

接口契约（照服务端 `server/index.ts` 与网页版对齐，别再自己猜）：`POST /api/chat`，
头只要 `Content-Type: application/json`（**没有鉴权头**，Key 在服务器侧）；
体 `{ model, messages }`，`messages[i]` 是 `{ role, content }`，角色值 `system`/`user`/`assistant`；
响应是标准 SSE，`data: {...}`，结束 `data: [DONE]`，增量在 `choices[0].delta.content`。
`stream` / `max_tokens` / `enable_thinking` 由服务端硬编码，客户端不用发。

### 9.2 `decem-core` 的 feature 改造

Makepad 版不该为一个用不上的 `reqwest` 在 Android/iOS 交叉编译时多背一层 rustls，
所以给 `decem-core` 加了 feature：

```toml
[features]
default = ["http"]
http = ["dep:bytes", "dep:futures-util", "dep:reqwest", "dep:tokio"]
```

关掉 `http` 就只剩纯逻辑（`models` / `message` / `prompt` / `sse`），第三方依赖只有 serde 三件套；
`provider` 模块与 `Error::Transport` 都挂在 `#[cfg(feature = "http")]` 下。
Makepad 侧用 `default-features = false` 引入。

**默认 feature 开着，egui 版 / Tauri 版行为一字未变**——已用 `cargo check -p decem-app`
与 `cargo test -p decem-core`（两种配置各跑一遍，17+4 个测试全过）验证。

### 9.3 实测（Windows，release 构建）

| 项 | 结论 |
| --- | --- |
| 发一条真实消息拿到回复 | ✅ 模型按人格回复，流式逐块出现 |
| 带代码块与表格的回复 | ✅ `CodeBlock` 带底色与 Copy 按钮；表格列对齐、表头有底色 |
| **Windows 原生 TLS** | ✅ **没撞证书吊销检查**（Windows 上走 WinHTTP），不需要换 rustls 客户端 |
| Esc 中断真实流 | ✅ 中断后内容立刻停住，再等 4s 截图字节完全相同（零增长） |
| 中文输入法 / 流式动画 / 气泡贴文字 | ✅ 与阶段 1 一致 |

### 9.4 已知缺口（阶段 3 或以后处理）

- **非 2xx 的错误路径没有真实触发测试**：`upstream_error()` 会从响应体里抠 `error` /
  `error.message` / `message` 并作为一条 char 消息展示，逻辑写了但没在真实 4xx/5xx 上验过。
- **流中途的错误事件被静默忽略**：若上游在流里塞 `data: {"error":...}`，`parse_sse_line`
  返回 `Ignore`。**网页版 / egui / Tauri 三个实现都一样**，不是这次引入的，但确实是缺口。
- **没有重试**（与现有实现一致）。
- **模型写死为 `DEFAULT_MODEL`**；图片消息、重新生成、清空记录、设置与持久化都还没做（阶段 3）。
- `Limits` 调到了 `total_timeout = 600s` / `max_body_bytes = 8MB`（默认 60s / 1MB 对长回复太紧）。
