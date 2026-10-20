# Makepad 迁移：状态、计划与作业手册

> 更新：2026-10-16　关联文档：`docs/makepad-probe.md`（探针实测记录与候选筛选）
> 这份文档的目的：**换一个会话、隔几周回来，也能立刻接着干**。

## 0. 一句话状态

方向已定：**用 Makepad（纯 Rust 自绘）重做 Decem Chat**，分四阶段、每阶段有验收门槛。
当前进度：**阶段 1（骨架）与阶段 2（真实网络）都已完成并实测通过**（`decem-makepad/`）。
骨架能跑、中文输入法能上屏、流式打字机正常、300 条长列表滚动跟手；
现在发出去的是真请求，`decem-core` 的 SSE 解析器接 `blocking_http` 的阻塞流，
Esc 能中断真实流。证据见 §3 与 §9。
**阶段 3 进行中**：图片消息已实装并实测（§10），剩下重新生成、清空记录、模型切换、设置与持久化。
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


### 阶段 3：功能对齐（进行中）
- ✅ 图片消息（base64 → 多模态请求 → 气泡里显示原图）：见 §10
- 待做：重新生成、清空记录（二次确认）、模型切换面板与「视觉理解」标注、
  设置（API Key / 接口地址）、本地持久化、头像、主题、Toast、打字速度

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
- **模型写死为 `DEFAULT_MODEL`**；重新生成、清空记录、模型切换、设置与持久化还没做（图片消息已完成，见 §10）。
- `Limits` 调到了 `total_timeout = 600s` / `max_body_bytes = 8MB`（默认 60s / 1MB 对长回复太紧）。

## 10. 阶段 3 之一：图片消息（2026-10-16，已实装）

### 10.1 通路

```
点「＋图」→ FileDialog（平台 native，异步）→ FileSelected { paths }
        → std::fs::read → 体积检查 → base64 成 data URL → 进待发槽位
        → 回车/点发送 → Message.image_url = data URL
        → decem-core::format_messages 按模型是否支持视觉拼成多模态数组
        → 用户气泡里用 ImageRef::load_image_from_data 显示原图
```

代码：`decem-makepad/src/main.rs` 的 `pick_image` / `handle_file_dialog` / `accept_image` /
`read_image` / `mime_for` / `to_data_url`，以及 `User` 模板里的 `photo := Image{...}`。

**实测**：发一张插画给模型，它准确描述了画面内容（抱着的虾片包装、额头的红色圆圈、爪子上的红色印子）
——说明图**真的发出去了**且模型**真的看到了**。截图在探针目录 `12-阶段3-图片消息-模型看到了.png`。

### 10.2 两个坑（都踩过，别再踩）

**① `Image` 不要写 `Fit` 轴，也别让"空图"参与布局——NaN 会毒掉整行（详见 §12）。**
把预览图作为输入行（`View{flow: Right}`）的一个子部件时，**`glass.TextInput` 会拿不到键盘焦点**：
点上去没有聚焦环、打字不进去，但整个 App 是活的（点按钮能开对话框）。
从截图能看出线索——输入框的描边从行的最左边开始、盖住了左边的按钮，说明 `Image` 的
`Fit` 宽度把这一行的布局搅乱了，输入框的**命中区**和画出来的框对不上。
移出到单独一行（根 `View{flow: Down}` 里、输入行上方）就恢复正常。

**② `FileDialog::want_bytes(true)` 这条路外部用不了。**
它回给应用的是 `FileDialogLoadAction`，而那个类型是 `platform` 的 `pub(crate)`
（`platform/src/file_dialogs.rs:317`），外部 crate 命名不到；全仓库 `apps/`+`examples/`
也**没有任何消费它的示例**（`grep want_bytes` 零命中）。
所以走公开的 `FileDialog` + `FileSelected { paths }` 拿路径，字节自己 `std::fs::read`。

### 10.3 已有的取舍与缺口

- 单张图上限 8 MB（服务端把请求体放宽到 20MB）；`Limits.max_body_bytes` 也调到 8MB。
- 气泡里的图 `max_height: 320`（逻辑像素）、`ImageFit.Smallest`（**不裁不拉伸**，
  整张可见）；所以特别竖长的图两侧会留白。点开看大图还没做。
- **图片只在气泡部件新建时解码一次**（用 `item_with_existed` 的第二个返回值判断）。
  `ImageRef::load_image_from_data` 每次都真解码，而这条路径每帧都会跑到；
  另外 `ImageRef::set_texture` 在 draw 事件里**会主动请求重绘**（`image.rs:1816-1818`），
  无条件每帧调就是持续重绘。
- `ImageAttachment.data_url` 与原始字节都留在 `Msg` 里（历史最多 10 条），没做清理。
- **`DECEM_IMAGE=<路径>` 启动钩子**：把一张图直接放进待发槽位，用来在**系统文件对话框
  无法被自动化驱动**（模态窗口，焦点会被 CU 覆盖层截走）的情况下验后半条链。
  与 `DECEM_STRESS` 同性质，不是给用户用的开关。

## 11. 阶段 3 之二：重新生成 + 清空记录（2026-10-16，已实装）

由子代理按 §10 同级别的侦察报告实现，我在真机上验证并修掉两处**只有运行期才暴露**的问题。

**重新生成**：最后一条 char 气泡旁边挂按钮（`PortalList` 逐项动作 + `set_visible` 控显隐），
流式时隐藏；`App::regenerate` 在锁内 `rposition` 找最后一条 user、`truncate` 到它、
用截断后的历史重新请求；`worker.is_some()` 守门（照 egui 版——网页版这里其实有缺陷）。
请求体拼装抽成 `build_request_body`，send / regenerate 共用。

**清空**：内置 `Modal` 二次确认（取消 / 确认清空；遮罩与 Esc 由 Modal 的
`can_dismiss` 免费提供）→ 先 `worker.cancel()` 再清 messages、只留欢迎语。
入口最初是输入行最左的「清空」按钮，**2026-10-18 已按规格 §5-4 移进设置抽屉**
（见 §22），`clear_chat` / `clear_confirm` 两段逻辑没有变。

### 11.1 两个「编译期看不见」的坑（务必记住）

1. **`Image` 没有 `align` 属性**。写了会报
   `[E] property align not defined on type`——**Splash 是运行期解析的，`cargo check` 看不见**，
   只能在 stdout 里看 `[E]` 行。所以改完 UI **必须启动一次并 grep `\[E\]`**。
2. **`Image` 的宽度必须是 `Fill`**。用 `Fit` 时它会占住一块高度却**不画纹理**
   （用户气泡变成一块空白橙色方块），看起来像"图没发出去"，其实只是没画。

## 12. `Image` 的 NaN 陷阱（§10.2 ① 的准确机制）

之前手册里把「输入框丢键盘焦点」归因成"把 `Image` 放进了输入行"，**这个因果是错的**：
`＋图` 按钮一直在那一行里、从未出问题。真正的机制是 **NaN 污染**，证据在
`widgets/core/src/image.rs`：

```rust
// 1176-1179  空图没有纹理 → 尺寸取 placeholder（默认 0）
(self.placeholder_width as f64 / dpi, self.placeholder_height as f64 / dpi)   // (0, 0)

// 1208
let aspect = width / height;          // 0.0 / 0.0 = NaN

// 1209  源码自己的注释：A Fit height peeks as NaN

// 1250-1256  fit: Smallest
ImageFit::Smallest => {
    let walk_height = rect.size.x / aspect;   // NaN
    if walk_height > avail_height { .. }      // NaN 比较恒为 false
    else { walk.height = Size::Fixed(walk_height); }   // ← 把 NaN 写进 walk
}
```

链路：空图 → `aspect` NaN →（`Fit` 轴的 `rect.size.x` 本身也是 NaN）→ `height = Fixed(NaN)`
→ 行是 `flow: Right`，**下一个子部件的位置由上一个的 rect 推出来**，NaN 顺着兄弟节点传播
→ `TextInput` 的矩形变 NaN → 命中测试 `rect.contains(abs)` 对 NaN 恒为 false
→ 永远收不到 `Hit::FingerDown` → 而 `TextInput` 唯一的聚焦入口就是那里
（`text_input.rs:3084-3093` 的 `self.set_key_focus(cx)`）。

同一条链解释了三个现象：

| 现象 | 同一个 NaN |
| --- | --- |
| 输入行里的空图 → 输入框丢焦点 | NaN 高度污染整行，命中区失效 |
| 气泡里 `width: Fit` 的图**占位却不显示** | `rect.size.x` 是 NaN → 同样 `Fixed(NaN)`，绘制 scale 也坏 |
| 纯文字气泡被撑高一大块 | 空图 NaN 高度落回"请求的高度"（`max_height`）→ 占位不画 |

✅ 两条修法：**宽度用 `Fill`（别用 `Fit`）** + **没有图时 `set_visible(false)`**
（`Image` 有 `#[visible]`，`!visible` 时既不画也不占位）。

延伸规则：任何一个部件的尺寸若可能来自"无边界的除法/比例计算"，都不要让它以 `Fit`
参与同行的布局；Makepad 没有 CSS 那种把 NaN 钳到 0 的兜底。
## 13. 背景不黑的两个原因（2026-10-17 已修）

窗口主体一度渲染成 `#4C4C4C` 而不是写死的 `#000000`，是两个独立问题叠加：

1. **裸 `View` 画不了纯色。** `View.draw_bg` 是 `DrawQuad`，而 `DrawQuad` 的脚本
   着色器 `pixel` 直接 `return #0000`，结构体里也没有 `color` 字段
   （`draw/src/shader/draw_quad.rs:65-67`；`view.rs:77`；`view_ui.rs:5` 的
   `mod.widgets.View = mod.widgets.ViewBase {}` 没有补着色器）。
   于是 `View{show_bg: true draw_bg.color: #x000000}` 会接受属性、一个像素都不画。
   Makepad 自己的注释说得更直白：`apps/storybook/src/shell.rs:110-114`
   （"a bare View's draw_bg has no `color`"）。
   ✅ 纯色底用 `SolidView`（`view_ui.rs:85`），圆角面板用 `RoundedView`。
2. **`desktop_style::install` 装晚了。** `makepad_widgets::script_mod` 的第一步就是
   按当前 StyleSheet 求值 `theme.splash`（`desktop_style.rs:281`），部件注册完再
   补 `widgets.splash`（`:297`）。在它之后再 install，整棵树拿到的仍是默认暗色：
   `theme.color_bg_app = mix(#000, #fff, 0.3) = #4C4C4C`
   （`theme_desktop_dark.rs:143-146,169-173`），窗口 clear 色也就停在这个灰
   （`window.rs:154`）上。black-orange 本来会把它改成 `#14171c`
   （`widgets/themes/black-orange/widgets.splash:6`）。
   ✅ install 必须放在 `makepad_widgets::script_mod` **之前**；
   Makepad 自己的 `apps/storybook/src/theme.rs:105-113` 也是这个顺序。

修法与验收：根底 `SolidView{draw_bg.color: #x000000}`，并给 `Window` 加
`pass +: { clear_color: #x000000 }` 兜住边缝。`System.Drawing` 采窗口截图，
`(100,300) / (1200,300) / (700,900) / (20,20)` 均为 `#000000`。

附带踩到的坑：`new_batch: true` 不要加在 `Modal` 的卡片上——实测按 Esc 关闭后
卡片与文字消失、按钮残留（overlay 复用了旧批次）；去掉即正常。

## 14. 本地时间不能靠 `makepad_platform::windows`（2026-10-17）

`makepad_platform` 确实 re-export 了 `windows` crate（`platform/src/lib.rs:169-170`），
但它的 `windows` 依赖**没有开 `Win32_System_Time` feature**
（`platform/Cargo.toml:141-188` 的 feature 列表里没有），
所以 `windows::Win32::System::Time::{GetLocalTime, GetTimeZoneInformation}` 在应用侧根本不存在。
给依赖补 feature 会改整个 `windows` 的编译图（`platform/Cargo.toml:136-140` 的注释就是为此写的），
为一行时间戳不值得。

✅ 做法与 Makepad 自己的 `tools/makepad_builder/src/tui.rs:2649-2657` 一致：
自己声明 extern，不引入新依赖：

```rust
#[link(name = "kernel32")]
extern "system" {
    fn GetLocalTime(time: *mut [u16; 8]); // SYSTEMTIME
}
```

另外，消息头像用的是 `Image` 自带的面片圆角
（`widgets/core/src/image.rs:289-296` 的 `border_radius` + `:207-224` 的像素路径），
把它放在固定 40×40 的 `RoundedView`（2px 描边）里即可裁成圆；
`Image` 仍要用 `width/height: Fill` 填满内框，**不要再写 `Fit`**（NaN 坑见 §12）。

## 15. 输入区改版（2026-10-17，已实装）

按 `docs/makepad-visual-spec.md` §4 重做输入区：外层全宽玻璃条 + 胶囊 + 珊瑚发送键 + 底部提示。
下面四个坑都是这次现踩的，别再重犯。

1. **`glass.Button` 不认 `color_disabled`。** `glass.Button` 是 `glass.CutButton`
   （`widgets/families/extras/src/glass_panel.rs:821`），它把 `pixel` 换成了裁剪版，
   只混合 `focus/hover/down`（`:861-878`），**没有 `disabled`**。所以
   `set_disabled(cx, true)` 的字会变灰、底却仍是珊瑚。发送键要用核心的
   `ButtonFlat`（完整着色器带 `disabled` 混合，`widgets/core/src/button.rs:1333-1346`
   的 `set_disabled` 驱动 animator 的 `disabled` 轨道）。
   ⚠️ 但 `ButtonFlat` 的 `draw_bg` **没有** `top_glint` / `inner_shadow`（那是 CutButton 的字段），
   写进 `draw_bg +:` 会在运行期报 `[E]`——`cargo check` 看不见。

2. **胶囊用 `Overlay`，发送键叠在 `TextInput` 上。** 胶囊高 56 = 40 内容 + 上下 8 内距；
   `TextInput` 用 `Fill/Fill` 铺满胶囊，发送键是 `Overlay` 的第二个子节点（40×40、右中）。
   View 默认 `event_order: Up`（`widgets/core/src/view.rs:41-47`），最后画的按钮先收事件，
   命中按下后 `t.handled` 会把事件占住，**输入框不会抢走发送键的点击**；
   文字区也不会被盖住，因为 `TextInput` 的右内距留了 8 + 40 + 8。
   `Image` 仍然不能进 `flow: Right`（§12 的 NaN）。注意：**按钮不是 `Image`**，
   「🖼」后来已按规格移进胶囊的 `flow: Right` 行（见 §22），输入框命中区正常。

3. **空 `Image` 的预览位会把输入区撑高一大截。** 旧布局里 `pending_photo` 是
   `width: Fit / height: Fit / max_height: 120`，空纹理时高度落回 120（§12），
   输入条被顶成 350px。修法有两条：`set_visible(false)`，
   以及**不要用 `Fit` 宽度**——改成固定 `width: 120 height: 80`
   （Image 在 Fit 宽度下占位不画纹理，同 §12 第二条）。显隐在
   `App::handle_event` 首帧 / `accept_image` / `send` 三处同步。

4. **聚焦外描边只能近似。** Makepad 没有 `box-shadow`，也画不出矩形外的描边。
   这里在胶囊的 `Overlay` 里放一圈 `visible: false` 的 `RoundedView`
   （`border_size: 2.0`、珊瑚 30%），靠 `TextInputAction::KeyFocus` /
   `KeyFocusLost`（`widgets/core/src/text_input.rs:3063` / `:2237`）切显隐，
   用 `actions.filter_widget_actions_cast::<TextInputAction>(uid)`
   （`widgets/core/src/widget.rs:1848`）过滤。实测聚焦时描边像素为
   `(92,58,58)`，正是 30% 珊瑚压在 `(27,27,27)` 的胶囊底上。

另外：Makepad 的十六进制颜色是 **RRGGBBAA**（不是 AARRGGBB）。写成
`#x4DF38181` 会得到一条绿描边，30% 珊瑚要写 `#xF381814D`。

## 16. 设置抽屉与模型切换（2026-10-17，已实装）

右侧设置抽屉 + 「切换模型」已落地（头像上传、深浅色模式留给下轮）。
抽屉外壳用 `Modal`：它 `on_after_apply` 把上报的 Walk 钉成 `empty`（`widgets/core/src/modal.rs:215-244`），
放 body 里当兄弟节点不占布局。贴右的做法是给实例覆盖根布局
`align: Align{x: 1.0, y: 0.0}`（Overlay 流的 align 见 `draw/src/turtle.rs:4908-4920`），
`content` 固定 320 宽、高度 `Fill`；点在 content 矩形外的遮罩仍会关闭
（`modal.rs:307-315` 只把「落在 content 外」当作 dismiss），所以不用自己写遮罩逻辑。

模型列表**没有用 `DropDown`**：它一行只有一段文字，做不出「名字 + 薄荷徽章 + 描述」
两行结构，也没法把对勾固定成灰色。改用 `ModelList`（自定义容器）+ `PortalList` +
自定义行 `ModelRow`，名字/描述/徽章全部来自 `decem_core::AVAILABLE_MODELS`，
UI 不另存一份。选中态由 `static MODEL_ID: RwLock<&'static str>` 驱动，
`build_request_body()` 每次请求时读取，所以下一条消息（含重新生成）立刻用新模型。

这次现踩的坑：

1. **`#[derive(Widget)]` 只生成 `WidgetNode`，不生成 `Widget::handle_event`**
   （trait 默认实现是空的，`widgets/core/src/widget.rs:302`）。自定义容器必须在
   `impl Widget` 里手写 `handle_event` 并转发给 `#[deref] view`，否则内层
   `PortalList` 收不到任何鼠标事件，行永远点不中。既有的「列表内重新生成按钮
   无响应」很可能也是这个根因（`ChatList` 同样没有转发）；本次按约定没有去改它。
2. **`visible: false` 仍然占布局。** 收起模型列表不能只 `set_visible(false)`，
   还要把高度压成 0：`widget.as_view().set_walk(cx, Walk{ height: Size::Fixed(0.0), .. })`
   （`ViewRef::set_walk`，`widgets/core/src/view.rs:361-369`）。否则会空出 320px。
3. **字体缺字只在运行期报。** Makepad 自带字体集缺 `U+25BE`（▾）与 `U+2715`（✕），
   日志是 `font miss U+25BE ...`，界面上是豆腐块，`cargo check` 完全看不见。
   折叠箭头改用 `▼`（U+25BC），关闭改用 `×`（U+00D7）即可。
4. **`items_with_actions` 会按“每条 action”返回 item。** 同一批 actions 里既有
   hover 的 `ViewAction`、又有行自己发的 click action，两者都带同一个 item uid，
   同一个 item 会被返回两次；直接在里面调 `select_model` 会执行两遍。
   先收集、最后只处理一次。
5. 行的 hover/click 直接读原始 `Event::MouseMove` / `Event::MouseUp` +
   `event.hits(cx, self.view.area())`，不要指望自定义部件根 View 的 `cursor`
   一定把 `ViewAction` 送出来。
6. 颜色仍是 RRGGBBAA：薄荷 20% 是 `#x95E1D333`，玻璃底是 `#x1a1a1acc`。

与规格的差异：无滑出动画（静态贴右）、无 backdrop blur（Makepad 没有）、
图标用文字字形（▼ / × / ⇄ / 👁）、深浅色模式仍是标题占位（§18 已实现）。

## 17. 更改头像与持久化（2026-10-17，已实装）

抽屉 §5 的第 2 段「更改头像」：64×64 圆形预览（2px `#F38181` 环）+
32px 高的「上传头像」按钮（圆角 10、1px 边框、16px 图标 + 8px 间距）。
点按钮弹系统文件对话框（`PICK_AVATAR`，过滤条件与图片消息的 `PICK_IMAGE` 完全一致），
选中后立刻换预览、顶栏头像，以及**后续**新建消息里的用户头像（已画出的旧消息不回溯）。

持久化：`%APPDATA%\DecemChat\avatar.<ext>`（`std::env::var_os("APPDATA")`，
目录不存在就 `create_dir_all`；扩展名跟随原文件，未知扩展名退回 `jpg`）。
换格式会删掉旧的 `avatar.*`，否则启动时按扩展名顺序可能读回旧图。
启动时优先读这个文件，覆盖 `public/images/user_01.jpg`；读失败会以 `⚠️` 消息提示，
不会静默吞掉。`DECEM_AVATAR=<路径>` 启动钩子与文件对话框走同一条
「读入 → 解码 → 替换 → 落盘」链路，用来绕过无法自动化的模态对话框。

这次现踩的坑：

1. **头像不能再用 `OnceLock`。** 上传后要能整体替换，改成
   `RwLock<Option<UserAvatar>>`（`bytes` + `custom`）。`custom` 同时决定顶栏画谁：
   网页版顶栏是 Decem（char）头像，这里保持默认 char、用户换过头像后才跟随用户头像。
2. **不要在 `ChatList::draw_walk` 的路径上推消息。** 它持有 `CHAT` 读锁，
   `push_message` 会再拿写锁——死锁。启动时的读盘错误只在
   `after_new_from_script`（已持写锁，直接 push 到 `chat.messages`）里提示。
3. **文件对话框要按 id 分流。** 两个对话框共用 `FileDialogAction`，
   `handle_file_dialog` 必须按 `PICK_IMAGE` / `PICK_AVATAR` 分别调用
   `accept_image` / `accept_avatar`，不能只认 id 之外就 `break`。
4. **上传按钮用透明 `ButtonFlat` 盖在「图标 + 文字」行上**（同模型触发器），
   这样 16px 图标与 8px 间距都能按像素摆；`ButtonFlat` 的 draw_bg 字段照
   §15-1 的清单写，别混入 `top_glint` / `inner_shadow`。
5. 预览图与顶栏图都是 `Image`：`fit: ImageFit.CropToFill`、`draw_bg.border_radius`
   裁圆；没有图时 `set_visible(false)`，不要留空纹理占位。

## 18. 深色/浅色模式与持久化（2026-10-17，已实装）

抽屉 §5-3：一行 `muted/50` 底、圆角 12、内距 12；左侧文字随模式变
「深色模式」/「浅色模式」，右侧开关轨道 32×18.4、滑块 16 圆（深色=开：
`#F18484` 轨道 + 黑滑块；浅色=关：`#262626` 轨道 + 白滑块）。
点整条轨道即可切换（透明 `ButtonFlat` 覆盖命中）。

### 18.1 切换机制（从源码确认）

Makepad 自带一条运行期换主题的通路，外部应用可以直接复用：

- `desktop_style::handle_event(cx, &Event::Custom(sheet.to_json()))`
  （`widgets/core/src/desktop_style.rs:303-319`）解析 `makepad_style` 信封，
  在 VM 里 `install()` 新 sheet，变化时调 `cx.request_style_reload()`。
- `request_style_reload()`（`platform/src/cx_api.rs:680-683`）置
  `pending_style_reload` + `pending_live_edit_request`。
- 事件循环随后投递 `Event::LiveEdit`；`app_main!` 的处理器
  （`platform/src/app_main.rs:327-348`）看到 `pending_style_reload` 时用
  `Apply::ScriptReapply` 重新执行 `AppMain::script_mod` 并重贴整棵部件树。
  `ScriptReapply` 会保留运行期 setter 写入的文字（输入框内容、我们的
  「深色模式/浅色模式」标签），也会保留动画。**这条通路可用**，
  参考实现：`apps/clock/src/main.rs:167-180`、`apps/weather/src/main.rs:187-200`
  （它们只是自己构造 `StyleSheet::load_with_appearance(...)` 再投递给
  `desktop_style::handle_event`）。

关键点：`AppMain::script_mod` 每次重载都会重跑，所以安装 sheet 的那一行必须
读当前模式（本项目用 `theme_stylesheet(theme_mode())`），不能写死
`BlackOrange`，否则 `handle_event` 装好的浅色 sheet 会被下一次重载盖回去。

`DesktopStyle` 没有 Light 变体（`desktop_style.rs:8-20`）；深色继续用
`BlackOrange`，浅色借 `DesktopStyle::Windows` 的 `mod.themes.light`
（`widgets/themes/windows/theme.splash:1`，令牌表在
`widgets/core/src/theme_desktop_light.rs`）。主题只负责 Markdown 正文、
Markdown 链接/行内代码、TextInput 光标与选区、GlassButton 这类主题驱动的部件；
界面自己的颜色全部由 Rust 的 `Palette` 两套值逐项覆盖。

### 18.2 颜色与持久化

`Palette`（`decem-makepad/src/main.rs` 顶部）把原来写死的深色全部成对给出浅色值：
根背景/clear `#000000 → #F2F2F2`、顶栏与抽屉玻璃 `rgba(26,26,26,.8) →
rgba(255,255,255,.8)`、char 气泡 `#1A1A1A → #FFFFFF`、输入区
`rgba(20,20,20,.8) → background/80`、胶囊 `rgba(38,38,38,.5) → muted/50`、
正文 `#FFF → #1A1A1A`、次级文字 `#B3B3B3 → #666666`、分隔线
`#333 → #D9D9D9`、遮罩 `rgba(0,0,0,.5) → rgba(0,0,0,.3)`。
用户气泡保持珊瑚渐变，并把 Markdown 正文/`font_color` 钉成白色；
代码块保持深色（与网页版一致）。

持久化：`%APPDATA%\DecemChat\theme`，内容 `dark\n` / `light\n`（与头像同目录）。
启动优先级：`DECEM_THEME=light|dark` > 文件 > 深色。环境变量**不落盘**，
只覆盖本次运行，避免自动化改掉用户选择。

### 18.3 这次现踩的坑

1. **`#xRRGGBBAA` 的字节序不能想当然。** Rust 侧 `rgba(hex)` 若按
   `0xAARRGGBB` 拆，`0x000000FF` 会变成「蓝色 + alpha 0」；窗口 clear 色
   忽略 alpha，整个界面会变成纯蓝。正确拆法是
   `r=(hex>>24), g=(hex>>16), b=(hex>>8), a=hex`。
2. **`GlassButton` 的文字默认是白色**（`glass_panel.rs:257`），不跟随
   light theme。浅色玻璃上必须显式给 `draw_text.color`：重新生成用珊瑚，
   确认弹层的两个按钮用当前模式的正文色。
3. **`ScriptReapply` 会保留字符串字段。** `ArcStringMut::script_apply`
   在 `preserves_runtime_state()` 时直接返回（`platform/src/arc_string_mut.rs:100-105`），
   所以开关两侧的文字要用 Rust `set_text` 写（首帧、打开抽屉、切换时各一次），
   不能只靠脚本里的字符串插值。
4. **自带字体缺 `☾`/`☀`**，会显示豆腐块。图标改用 emoji `🌙` / `☀️`
   （与 ✨/🦊 同一套字形）。
5. 浅色下模型列表/抽屉的**文字是 Rust 每帧写的**（`ModelRow::draw_walk`），
   那里也要读 `palette()`，否则行文字仍是白色（白底白字）。

### 18.4 验证

- `cargo check`、`cargo clippy --release --all-targets -- -D warnings`、
  `cargo build --release`、`cargo fmt --check` 全绿。
- `DECEM_THEME=light` / `dark` 各启动 8 秒后结束，日志均无 `[E]`。
- 截图 + PowerShell `System.Drawing` 采像素：
  浅色空背景 `#F2F2F2`、顶栏玻璃 `#FCFCFC`、char 气泡 `#FFFFFF`、
  输入胶囊 `#ECECEC`；切到深色后背景 `#000000`、顶栏 `#151515`、
  char 气泡 `#1A1A1A`、输入胶囊 `#1B1B1B`。
- 切换后输入框里的文字保留；点击输入框可输入，Enter 发送、SSE 流式回答正常。
  重启（不带环境变量）后仍是上次的模式。

已知限制：浅色主题借用 Windows 的 token 表（字号/间距等非颜色令牌与深色
略有差异）；没有 backdrop blur；代码块在浅色下仍是 tokyo-night 深色
（与网页版一致）；`GlassButton` 的玻璃折射效果在浅色下偏白，但文字已保证可读。

## 19. 应用图标与 Android 打包的一个坑（2026-10-17）

### 图标

`cargo_makepad` 自带图标支持，不需要 rc.exe / windres：

| 平台 | 放哪 | 期望文件 |
| --- | --- | --- |
| Windows（exe 资源） | `<crate>/resources/` | **必需** `icon_32.png`、`icon_64.png`、`icon_128.png`、`icon.ico`；可选 `icon_256/512/1024.png`（`utils.rs:792-832`） |
| macOS | 同上 | `icon.icns`，缺了就用 `icon_1024.png` 兜（`desktop.rs:165-173`） |
| Android | `<crate>/resources/android/res/` | `mipmap-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/ic_launcher.png`（48/72/96/144/192），整目录会被拷进 APK（`android/compile.rs:1062-1090`） |

⚠️ **Windows 图标只在 `cargo makepad desktop build` 时链进 exe**。之后如果用普通
`cargo build` 因为源码改动触发了重新链接，图标会丢——出正式包请统一走 `cargo makepad`。

### 坑：`decem-makepad/Cargo.toml` 里不能写 `[workspace]`

`cargo makepad android` 会生成一个包装 crate 并在它的 manifest 末尾追加 `[workspace]`；
我们的 `Cargo.toml` 如果已经有一份，就变成 `duplicate key`，Android 打包直接失败：

```
error: duplicate key  --> Cargo.toml:26:2  | [workspace]
```

**隔离靠根 `Cargo.toml` 的 `exclude = ["decem-makepad"]`**（那条才是生效的）。
删掉 crate 里的 `[workspace]` 后：桌面构建照常、`cargo metadata --no-deps` 里仍然
不含 `decem-makepad`（未被并入主 workspace），`Cargo.lock` 依然独立。

出包命令（`-p` 必填）：

```bash
cd decem-makepad
cargo makepad android --abi=aarch64 --sdk-path='D:/Android/Sdk' \
  --package-name=top.valedecem.decemchat --app-label='Decem Chat' \
  build -p decem-makepad --release
```

产物：`target/android/makepad-android-apk/decem_makepad/apk/decemchat.apk`

## 20. 视觉对齐第二轮：背景光晕 / 重新生成链接 / 发送键（2026-10-18）

对应 `docs/makepad-visual-spec.md` §1、§3、§4。只改了 `decem-makepad/src/main.rs`。

### 20.1 背景光晕：一次全屏 quad 里做解析式径向渐变

网页版是三个绝对定位的 `div`，各自 `radial-gradient(circle, rgba(...,0.4) 0%, transparent 70%)`
加 `filter: blur(40/50/60px)`（`src/components/ChatInterface.tsx:75-99`），
再叠 `animate-pulse-glow`（3s，opacity 0.3↔0.6、scale 1↔1.1，`src/index.css:185-190`）。

Makepad 侧的做法：

- 新增 `Backdrop` 部件（`main.rs` 的「背景光晕」段）替换原来的根 `SolidView`。
  它先用自己的 `DrawBackdrop` 着色器画「底色 + 三层 orb」，再把 walk 交给内部
  `#[deref] view`。所以光晕**在任何子内容之前绘制**，天然位于消息/顶栏/输入区之下；
  它也不对自身 draw 区域调用 `event.hits`，不会抢命中。
- 渐变不是贴图、也不是离屏 blur：`pixel` 里直接算
  `alpha = peak × mix(0.5,1.0,pulse) × exp(-2.7·t²)`，`t = 距离 / 半径`。
  `exp(-2.7t²)` 是对「圆锥剖面（0% 处 0.4 → 70% 处 0）+ 高斯模糊」
  做数值卷积后的拟合：t=0.5/0.7/1.0 时相对值 ≈0.51/0.26/0.06。
  因此没有真正的 60px 模糊半径，但边缘同样是平滑的高斯状衰减。
- 位置/尺寸：珊瑚 384、左 1/4 上 1/4；薄荷 320、右 1/4 下 1/4；黄 256、右 1/3 垂直居中。
  全部按 `rect_size` 实时算，窗口缩放时比例不变。
- 呼吸：相位在 Rust 侧 `glow_seconds()`（进程级 `OnceLock<Instant>`，切主题不重置），
  着色器把相位换成 ease-in-out 三角波。`App` 用 `cx.start_interval(0.1)` 以 10 Hz
  触发 `redraw_all`。没有用 60 Hz：对这层 3–6% 的光晕没必要，也不让 GPU 常亮。
- 峰值 alpha：珊瑚 6%、薄荷 4.5%、黄 3%。这是**最终叠到背景上的 alpha**
  （网页版的 orb opacity 与动画 opacity 相乘、再被 blur 摊薄后的量级，规格 §1/§8-4）。

实测（PowerShell `System.Drawing` 采样，窗口 1470×1140 截图）：

| 主题 | 珊瑚中心 | 薄荷中心 | 黄中心 | 纯背景 |
| --- | --- | --- | --- | --- |
| 深色（峰值） | `#0F0909` | `#090B0B` | `#0C0A07` | `#000000` |
| 深色（低相位） | `#090606` | `#090908` | `#070705` | `#000000` |
| 浅色（峰值） | `#F2EBEB` | `#EFF0EF` | `#F1F1EE` | `#F2F2F2` |
| 浅色（低相位） | `#F1EEEE` | `#EFF0EF` | `#F1F1EE` | `#F2F2F2` |

深色与浅色各做过一次同点跨 3s 的 32 次采样（100ms 间隔）。深色珊瑚 G 通道
6↔9、浅色 235↔238 往复，说明 3s 呼吸在跑。深色中心峰值 `#0F0909`
（相对纯黑 15/255≈5.9% 的像素值），按珊瑚通道反推 alpha≈15/243≈6.2%，
低相位 `#090606` 约 3.5%；正文/气泡对比度不受影响。

### 20.2 「重新生成」改成文字链接

规格 §3 要求 12px 珊瑚文字、12px 图标 + 4px 间距、无边框无底色。
`ChatList` 用 `glass_button(...)` 按类型查找按钮（`main.rs::draw_walk` / `handle_actions`），
所以不能把 `glass.GlassButton` 换成别的部件；改为把它的玻璃层整体关掉：

```text
draw_glass +: { layer_opacity: 0.0 surface_alpha: 0.0 shadow_alpha: 0.0 border_width: 0.0 }
```

`layer_opacity` 乘在 sdf 最终输出上（`widgets/core/src/gauss_view.rs:808`），
底、边、阴影、ripple 一起消失，只剩 `draw_text`。文字改成
`"↻ 重新生成"`（自带字体有 U+21BB，截图确认不是豆腐块），字号 12、颜色 `#F38181`。
显隐条件（最后一条 char 且非流式）与点击动作一行未改。

已知差异：网页版 hover 变 `#e86a6a`，这里拿不到 GlassButton 的 hover 颜色通道，
用 `hover_ink: 0.92` 做轻微变暗代替（深色下可感知，浅色下会略变淡）。

### 20.3 发送键

结构本来就是对的：40×40、圆角 16、可用态 `#F38181` + 白箭头；禁用态
`#262626`（50% alpha）+ 灰字（50% alpha），等价于整体 opacity 0.5。
截图采样：禁用圆块 `#202020`，胶囊底 `#1B1B1B` —— 只是略深一档，没有「实心黑块」
的问题，因此未改。胶囊结构与 `TextInput` 命中区未动。

### 20.4 验证

- `cargo check`、`cargo clippy --release --all-targets -- -D warnings`、`cargo fmt --check` 全绿。
- `cargo makepad desktop build -p decem-makepad --release` 出包（保住 exe 图标）。
- `DECEM_THEME=dark` / `light` 各启动 7 秒后结束，stderr 无 `[E]`。
- 交互回归（深色）：点输入框 → 打字 → Enter 发送 → SSE 回复正常；
  点「↻ 重新生成」→ 最后一条回复被替换，证明 18px 高的文字链接仍能命中。

## 21. 代码块圆角 + 顶栏图标去 emoji（2026-10-18）

只改了 `decem-makepad/src/main.rs`。

### 21.1 代码块圆角

网页版 `pre` 是 `bg-black/30 rounded-lg p-3`（`src/components/MessageBubble.tsx:105`），
视觉上落在 Tokyo Night 的 `#1a1b26` 上、圆角 12、内距 12。
Makepad 的 `CodeView` 自己画不出圆角：它的底色是 `DrawColor` 直角色块
（`code_editor/src/code_editor.rs:250`），没有 radius 参数。

做法：把 `code_block` 从 `View` 换成 `RoundedView`，底色与半径画在外层，
`CodeEditor.draw_bg` 置全透明（`#x00000000`），圆角就露出来了：

```text
code_block := RoundedView {
    width: Fill height: Fit
    show_bg: true
    draw_bg.color: #x1a1b26
    draw_bg.border_radius: uniform(12.)
    padding: Inset{left: 12 top: 12 right: 12 bottom: 12}
    code_view := CodeView { ... editor +: { draw_bg +: { color: #x00000000 } } }
}
```

**内距不会叠加**：`CodeViewBase` 已经把 `pad_left_top` 设成 `vec2(0,0)`
（`code_editor/src/code_view.rs:11-12`），所以 12 的内距只来自外层 `RoundedView`。
`Markdown` 仍按 `ids!(code_view)` 找编辑器（`markdown.rs:456`），
`widget()` 在子树里递归查，外面多包一层不影响。

⚠️ 一个**不是 bug 的观感问题**：模型偶尔把语言标记和代码写在同一行
（` ```rust fn answer() -> i32 { 42 } `）时，pulldown-cmark 会把整行当作 info string，
代码内容为空 —— 界面表现为一个空代码块。是否复现取决于模型输出格式，
与这层圆角包装无关（seed 同字符串可稳定复现空块）。

### 21.2 顶栏齿轮 / 眼睛图标：改用 `theme.font_icons`

Makepad 自带的 `Icon` 部件要 `crate_resource("self:resources/xxx.svg")`：
要么往 `resources/` 加文件（双击 exe 的 cwd 不稳定，资源目录会变成运行期依赖），
要么自己注册 iconset。两条都超出「轻量」范围，**没有走 SVG**。

> **已被 §25 取代**（2026-10-18）：SVG 路线后来走通了，顶栏设置与 GitHub
> 现在都是 lucide 的描边 SVG 原图。§21.2 只记录当时的取舍。

改用主题里已有的 FontAwesome Solid（`theme.font_icons`，
`widgets/core/src/theme_desktop_dark.rs:677-683`）：单色字形、跟随 `color` / `color_hover`。

| 位置 | 原来 | 现在 |
| --- | --- | --- |
| 顶栏设置按钮 | `⚙︎` emoji（蓝，染不上色） | `\u{f013}` fa-gear，18px，`palette().muted_fg`，hover 变 `palette().fg` |
| 抽屉「当前」徽章 / 模型行徽章 | `👁` emoji | `\u{f06e}` fa-eye，11px，`#x4DB6AC` |

~~顶栏 `✨` 留 emoji~~：这条判断后来证明是错的 —— `fa-solid-900.ttf` 的 cmap 里
**有** `fa-wand-sparkles`（U+F72B），只是当时没查。`✨` 与抽屉里剩下的
`⇄ 👤 🌙 🗑`、输入胶囊的 `🖼` 已在 §24 一并换成单色字形。

### 21.3 验证

- 四角像素（PowerShell `System.Drawing`，真实网络回复的代码块）：
  深色气泡 `#1A1A1A` 内，代码块边界 `#1A1B26`；四角点
  `(269,447)/(1055,448)/(270,837)/(1055,837)` 全部为 `#1A1A1A`，
  同边向内 30px 处为 `#1A1B26` —— 圆角真的把底色切掉了。
  浅色同样：角点 `#FFFFFF`，边内 `#1A1B26`。
- 齿轮取色：顶栏齿轮墨色平均 `#AFAFAF`（`#B3B3B3` + 抗锯齿），不再是蓝色 emoji。
- `cargo check` / `cargo clippy --release --all-targets -- -D warnings` / `cargo fmt --check` 全绿；
  `DECEM_THEME=dark`、`light` 各启动 7 秒，日志无 `[E]`、无 `font miss`。
- 交互回归：点输入框 → 中文输入 → Enter 发送 → SSE 逐字回复 → 代码块圆角/内距正确。

## 22. 输入胶囊与「清空」迁移（2026-10-18）

只改 `decem-makepad/src/main.rs`，目标是规格 §4 / §5 的布局：

### 22.1 胶囊内部 `[🖼] [input Fill] [↑]`

图片按钮从胶囊左侧外部挪进胶囊内的 `flow: Right` 行，放在 `TextInput` 之前；
顺序是 `attach_button`（40×40）→ `input`（Fill）→ `send_button`（40×40）。
§12 的 NaN 只来自 `Image`（`Fit` 轴 / 非输入部件的图片纹理），
`glass.Button` 进输入行不会碰坏 `TextInput` 的命中区——实测点输入框能聚焦、
能打字、Enter 能发（见 §22.4）。

配色/形状不变：40×40、圆角 16、透明底、hover `rgba(243,129,129,0.1)` + 珊瑚字。
胶囊本身仍是 56 高（40 内容 + 上下 8 内距）、`row_bg`、圆角 16；
外层 `Overlay` 只剩聚焦描边这一层。

### 22.2 抽屉第 4 段「清空聊天记录」+ 页脚

按 §5 的段序放在「深色/浅色模式」之后：

- 段标题：`🗑`（`#EF4444`，16px）+「清空聊天记录」（`palette().fg`，14px）
- 按钮 `drawer_clear_button`：`glass.Button`，`height: 32`、圆角 10、
  底 `#EF4444`（hover `#DC2626` / down `#B91C1C`）、白字 13px、文字「清空记录」
- 按钮下方 12px `#B3B3B3`：「此操作不可撤销，将删除所有聊天记录」
- 点击仍开**原来那个** `clear_confirm`；取消/确认的处理与 `clear_chat` 完全复用

页脚「Ave atque vale」：顶部分隔线 + 18px 居中文字。网页版是
`background-clip: text` 的珊瑚↔薄荷↔黄渐变 + 4s 位移/呼吸；Makepad 没有
渐变裁剪文字，改成沿同一条渐变取一个**随时间移动的单色**（`footer_glow()`，
4s 一循环）。重绘由既有的 10 Hz 光晕定时器驱动，不额外开定时器。

抽屉内容变长后仍能在 980×760 窗口里完整放下（正常状态不滚动），
模型列表展开时照旧会顶出内容（§16 的既有行为）。

### 22.3 双 Modal 的层级：`clear_confirm` 必须声明在抽屉之后

`Modal` 的 overlay 按**绘制顺序**叠放（`widgets/core/src/modal.rs` 的双 modal
测试：后声明的那层在上面；`draw_walk` 里 `begin_overlay_reuse` 后直接画自己的
scrim + content）。原来 `clear_confirm` 写在 `settings_drawer` **前面**，
抽屉开着时把它整个盖住，按钮点不到。

修法：把 `clear_confirm` 整块移到 `settings_drawer` 之后（仍是 Backdrop 的兄弟）。
两个 modal 可以同时 open，关掉上面那个后下面的抽屉还在，取消清空后聊天记录保留。

### 22.4 验证

- `cargo check`、`cargo clippy --release --all-targets -- -D warnings`、`cargo fmt` 全绿。
- `cargo makepad desktop build -p decem-makepad --release` 出包（**必须在
  `decem-makepad/` 目录里跑**；在仓库根跑会报 `Failed to get crate dir`，
  因为根 workspace 把该 crate `exclude` 了）。
- `DECEM_THEME=dark` / `light` 各启动 7 秒后 kill，日志无 `[E]`。
- 胶囊像素（PowerShell `System.Drawing`，1470×1140 截图）：
  `x=5 → #101010`（输入区底）、`x=20/30/45/90/100/200 → #1B1B1B`（胶囊底）、
  `x=60 → #89C2AC`（🖼 字形本身），说明按钮整块落在胶囊背景范围内；
  `x=1420 → #202020`（禁用发送键）、`x=1456 → #101010`（胶囊外）。
- 交互：点输入框 → 输入「布局自检：胶囊与抽屉」→ Enter 发出 → SSE 回复正常；
  点胶囊内 `🖼` 弹出系统「选一张图」对话框，Esc 取消；
  设置抽屉 → `清空记录` → 二次确认压在抽屉之上 → 取消 → 聊天记录仍在；
  深浅色切换（深→浅→深）正常，红色按钮采样 `#EF4444`。
- 残留差异：无渐变文字、无 backdrop blur；清空区的垃圾桶现在是 FA
  `fa-trash-can`（§24），与网页版的 lucide Trash2 形状仍不同（色值一致）。

## 23. 拉丁字体对齐网页版：Lato / Poppins / Fira Code（2026-10-18，已实装）

网页版三族字体（`src/index.css:1`）：正文 Lato、标题 Poppins、代码 Fira Code。
Makepad 侧的做法：**字体字节 `include_bytes!` 进二进制 → 脚本侧 `res.binary_resource`
注册成资源 → 改主题的 `font_family`**。中文与 emoji 靠保留原字体链成员。

### 23.1 应用侧能注册自定义字体（证据）

| 想干的事 | API | 位置 |
|---|---|---|
| 把内存里的字节变成脚本资源 | `res.binary_resource(u8_array)` | `platform/src/script/res.rs:1186-1190`（`id_lut!(binary_resource)` 起） |
| 资源路径 → 字体字节 | `Cx::get_resource_font_bytes_by_path` | `platform/src/cx_api.rs:1043`：先读文件，失败再取 `binary://…` 的 `Loaded` 字节 |
| `FontMember.res` → 成员 | `FontFamily` 的 `on_custom_apply` 用 `path_for_handle` 取 `abs_path` | `draw/src/shader/draw_text.rs:3836-3898` |
| 主题字体来源 | `install_theme_fonts` 把 `font_regular/bold/…` 重写成 `FontSet` 的链 | `widgets/core/src/font_policy.rs:141` |
| 深/浅色主题的字体表 | `font_regular` / `font_bold` / `font_code` / `font_icons` | `widgets/core/src/theme_desktop_dark.rs:646-681` |
| 国际字体链（中文/emoji 的兜底） | `INTERNATIONAL_REGULAR` = IBM Plex → LXGW → NotoColorEmoji | `platform/src/font_policy.rs:338` |

Rust 侧把 `Vec<u8>` 变成脚本数组：`vm.bx.heap.new_array_from_vec_u8(bytes)`，
再 `vm.set_injected_global(id!(...), value)` 让 `script_eval!` 里能按名字取到。
**不要**用 `script_eval!(vm, { mod.decem_fonts })` 探测是否已注册——读一个不存在的
属性会打 `[E] property ... not found`，直接踩掉"日志无 `[E]`"的验收。

### 23.2 格式：只能 TTF/OTF，不能 woff/woff2

`draw/Cargo.toml:21` 的 `ttf-parser` 只开了 `gvar-alloc`，没有 `woff`/`woff2` feature；
`FontFace::from_data_and_index` 会直接解析失败，该成员被当成不可用丢掉。
Google Fonts 的 `css2` 接口现在对任何 UA 都发 woff/woff2，所以取回来要用 fontTools
转回 TTF（`TTFont(x.woff).save(y.ttf)`，`flavor = None`）。

拿的是 **latin 子集**（`unicode-range: U+0000-00FF, …U+2000-206F, U+2191, U+2193…`）：
`→`(U+2192)、`✓` 之类不在里面，所以字体链里**必须保留 IBM Plex** 兜符号，否则变豆腐块。

### 23.3 实现（`decem-makepad/src/main.rs`）

- 四个常量 `LATO_REGULAR_TTF` / `LATO_BOLD_TTF` / `POPPINS_SEMIBOLD_TTF` /
  `FIRA_CODE_REGULAR_TTF`（`include_bytes!("../resources/fonts/…")`）。
- `install_web_fonts(vm)`：
  1. `res.binary_resource` 建 `mod.decem_fonts`（lato_regular / lato_bold /
     poppins_semibold / fira_code）。用 Rust 侧 `OnceLock` 保证只建一次——
     样式重载会重跑 `script_mod`，但 heap 不重置（`ScriptVm::with_reload` 只翻 flag，
     `platform/script/src/vm.rs:551`），重复建会每次切主题多留一份字体字节。
  2. 用 `mod.theme.font_regular{font_family: FontFamily{…}}` **合并**式赋值
     （不要新建 `TextStyle{…}`，否则字号/行距被重置）。
- 链顺序（每个族都一样）：自定义拉丁字体 → IBM Plex（符号兜底）→ LXGW（中文）→
  Noto Color Emoji（彩色 emoji）。四者都是 **eager 成员**，和
  `FontSet::International` 原来的行为一致，不依赖 lazy 加载。
- `font_code` = Fira Code → LiberationMono → LXGW → emoji。
- 新增主题 token `theme.font_display`（Poppins）只给顶栏「Decem」和抽屉「设置」两个
  标题用——网页版也只有 h1/h2 用 Poppins（`ChatInterface.tsx:117`、
  `SettingsPanel.tsx:89`），正文/强调仍是 Lato 700。
- **调用时机**：`install_web_fonts` 必须在 `makepad_widgets::script_mod` 之后
  （主题与 `mod.res` 已就绪）、在 `makepad_code_editor::script_mod` **之前**
  （CodeView 在那一刻就把 `theme.font_code` 抄进自己的 TextStyle），
  更不能晚于本 crate 的 `script_mod!`。

### 23.4 验证（2026-10-18）

- `cargo check` / `cargo clippy --release --all-targets -- -D warnings` / `cargo fmt` 全绿；
  `cargo makepad desktop build -p decem-makepad --release` 出包。
- `MAKEPAD_TRACE_FONT_LOAD=1` 的加载日志确认字体真的来自内存：
  `member=lato path=binary://1 bytes=72312`、`lato … binary://2 bytes=70576`、
  `poppins … binary://3 bytes=151516`；代码块首次出现时 `fira … binary://4`。
  中文/emoji 仍是 `LXGWWenKaiRegular.ttf`（19 MB）与 `NotoColorEmoji.ttf`（10.6 MB）。
- `DECEM_THEME=dark` / `light` 各跑 7 秒后 kill：`[E]` 0 条、
  `font miss`/`missing font`/`not packaged` 0 条。
- `DECEM_STRESS=1` + 浅色：300 条「压测第 N 条：中文、全角标点？！～、emoji 🦊…」
  全部正常——中文不是方块、🦊 仍是彩色（像素最大通道差 238）。
- 截图采样顶栏「Decem」：175 种颜色、亮度 21…255，说明是抗锯齿的真实字形
  （Poppins 的几何风格 D/e/m 与 IBM Plex 明显不同），不是纯色块或豆腐框。

### 23.5 剩余差异

- 只有 latin 子集：拉丁扩展、箭头等少见符号仍由 IBM Plex 出字形，字重/字形和
  网页版不完全一致。
- 网页版 `font-weight: 500/600` 的中间字重没有：Makepad 只按成员选字体，
  `font_bold` 固定用 Lato Bold 700 或 Poppins SemiBold 600。
- 斜体（`font_italic` / `font_bold_italic`）仍是 IBM Plex Italic，没有换 Lato Italic。
- 代码块里的中文注释走 LXGW（Fira Code 无 CJK），行高和网页版略有差别。

## 24. 剩余彩色 emoji 图标全部换成 FontAwesome 单色字形（2026-10-18）

只改 `decem-makepad/src/main.rs`。做法与 §21.2 相同：`text: "\u{fxxx}"` +
`text_style: theme.font_icons{...}`，`color` / `color_hover` 直接染单色字形，
尺寸与布局不变。

> **本节图标里的 3 个已被 §25 换成 lucide SVG**（wand-sparkles → `sparkles.svg`、
> shuffle → `bot.svg`、image → `image.svg`）。user / sun / moon / trash-can
> 仍是 FontAwesome 字形。

### 24.1 字形存在性（先查再写）

用 `fontTools` 读主题实际加载的那份字体
`cargo/git/checkouts/makepad-ec2f134f34cd9f98/71c9ddc/widgets/resources/fa-solid-900.ttf`：

```text
0xf007 True user
0xf185 True sun
0xf186 True moon
0xf2ed True trash-can
0xf074 True shuffle
0xf72b True wand-sparkles
0xf03e True image
```

（`f72b` 说明 §21.2 里「FA 免费集没有 sparkles」的判断是错的。）

### 24.2 替换表

| 位置 | 原来 | 现在 |
| --- | --- | --- |
| 顶栏标题旁 | `✨` 彩色 emoji | `\u{f72b}` fa-wand-sparkles，16px，`#xFCE38A` |
| 抽屉「切换模型」 | `⇄`（普通文字箭头，不是 emoji，但缺字会成豆腐块） | `\u{f074}` fa-shuffle，16px，`#xF38181` |
| 抽屉「更改头像」 | `👤` emoji | `\u{f007}` fa-user，16px，`#x95E1D3` |
| 抽屉「深色/浅色模式」 | `🌙` / `☀️` emoji（Rust 侧 `sync_theme_ui` 写入） | `\u{f186}` fa-moon / `\u{f185}` fa-sun，16px，`#xFCE38A` |
| 抽屉「清空聊天记录」 | `🗑` emoji | `\u{f2ed}` fa-trash-can，16px，`#xEF4444` |
| 输入胶囊「图片」按钮 | `🖼` emoji | `\u{f03e}` fa-image，18px，`palette().muted_fg`，hover `#F38181` |

关键点：**`theme_mode_icon` 的 `text_style` 必须同时从 `font_regular` 换到
`font_icons`**，否则 Rust 写进去的 `\u{f186}` 会用 Lato 渲染成豆腐块。

抽屉页脚只有 `Ave atque vale` 文字（`footer_glow()` 取色），没有 emoji，未动。

### 24.3 验证

- `cargo check`、`cargo clippy --release --all-targets -- -D warnings`、
  `cargo fmt`、`cargo makepad desktop build -p decem-makepad --release` 全绿。
- 深色 / 浅色各启动 7 秒后 kill，日志 `grep -a "\[E\]"` 与
  `grep -ai "font miss"` 均为空。
- 截图（kimi-cu，1470×1140）逐个确认：wand-sparkles / shuffle / user / moon /
  sun / trash-can / image 都是单色字形，没有豆腐块，也不是彩色 emoji。
- 像素采样（PowerShell `System.Drawing`，按区域背景色做阈值取「墨色」像素，
  再统计色相直方图）：每个图标的墨色**全部落在同一个 30° 色相桶**里，
  深/浅两套模式均如此——即单色或抗锯齿混色，不存在彩色 emoji 的多色调。

| 图标（浅色模式） | 墨色均值 | 色相桶 |
| --- | --- | --- |
| 顶栏 sparkle | `#B09E62`（被抽屉遮罩压暗的 `#FCE38A`） | 30° ×247 |
| shuffle | `#F38989` | 0° ×348 |
| user | `#96E1D3` | 150° ×420 |
| sun | `#FCE38D` | 30° ×512 |
| trash-can | `#EF5050` | 0° ×638 |
| image | `#4B4B4B` | 0° ×615 |

- 交互回归：齿轮开抽屉、深浅色来回切、点图片按钮弹窗，均正常；主题切回深色后退出。

## 25. 图标换成 lucide SVG 原图（2026-10-18）

只改 `decem-makepad/src/main.rs`，新增 `decem-makepad/resources/icons/*.svg`。
**之前所有「Makepad 没有图标集，只能用 FontAwesome 字形」的结论（§21.2、§24）
只适用于那一轮**：`Icon` 部件的 `draw_icon.svg` 与 `ButtonFlat` 的
`draw_icon` 槽位都能直接吃 SVG，形状可以与网页版逐像素一致。

### 25.1 六个图标

SVG 取自 lucide **0.562.0**（与网页版 `package.json` 的 `lucide-react@^0.562.0`
同版本），URL 形如
`https://raw.githubusercontent.com/lucide-icons/lucide/0.562.0/icons/send.svg`，
原文件写进 `resources/icons/`，未改路径数据。许可 ISC。

| 位置 | 原来 | 现在（lucide 原图） |
| --- | --- | --- |
| 标题旁星芒 | `\u{f72b}` fa-wand-sparkles 16px | `sparkles.svg`，16px，`#FCE38A` |
| 顶栏 GitHub | 文字 `GH` | `github.svg`（octocat 描边），20px，`palette().fg` |
| 顶栏设置 | `\u{f013}` 实心 fa-gear 18px | `settings.svg`（描边齿轮），20px，`palette().fg` |
| 输入胶囊图片键 | `\u{f03e}` fa-image 18px | `image.svg`，20px，`palette().fg` |
| 发送键 | 文字 `↑` 18px | `send.svg`（纸飞机），16px，白色 |
| 抽屉切换模型 | `\u{f074}` fa-shuffle 16px | `bot.svg`（小机器人），16px，`#F38181` |

写法（图标在按钮里时用按钮自带的 `draw_icon` 槽位，不要另叠一层 `Icon`：
后画的部件会抢走命中区，按钮就点不到了）：

```text
github_button := glass.Button {
    text: ""
    icon_walk: Walk{width: 20.}
    draw_icon +: { svg: crate_resource("self://resources/icons/github.svg") color: #(palette().fg) }
    ...
}
```

纯展示处用部件 `Icon { size: 16, draw_icon +: { svg: ..., color: ... } }`。

### 25.2 图标已嵌进二进制（2026-10-18 第二轮）

**问题**：`self:` 的解析见 `platform/src/script/res.rs:842-905`，它把
`script_mod.cargo_manifest_path`（**编译期的绝对路径**）与相对路径拼起来，
运行期按这个绝对路径读磁盘。第一轮用 `crate_resource("self://resources/icons/*.svg")`
时实测：把 `decem-makepad/resources/icons` 改名后重跑 release exe，六个图标
**全部消失**（日志里没有报错，只是空着）——exe 换机器/换目录就丢图标。

**现在**：六个 SVG 用 `include_bytes!` 嵌进二进制，走与字体相同的注入路线
（`install_icons`，`main.rs`）：

```rust
const ICON_SPARKLES_SVG: &[u8] = include_bytes!("../resources/icons/sparkles.svg");
// …

fn install_icons(vm: &mut ScriptVm) {
    static EMBEDDED: OnceLock<()> = OnceLock::new();
    if EMBEDDED.get().is_none() {
        for (name, bytes) in [(id!(sparkles_svg), ICON_SPARKLES_SVG), …] {
            let bytes: ScriptValue = vm.bx.heap.new_array_from_vec_u8(bytes.to_vec()).into();
            vm.set_injected_global(name, bytes);
        }
        script_eval!(vm, {
            mod.decem_icons = { sparkles: mod.res.binary_resource(sparkles_svg) … }
        });
        let _ = EMBEDDED.set(());
    }
}
```

DSL 里改成 `svg: mod.decem_icons.sparkles`（不再是 `crate_resource`）。

关键点：

- **调用时机**：`install_icons(vm)` 在 `AppMain::script_mod` 里紧跟
  `install_web_fonts(vm)`、在 `self::script_mod(vm)` 之前——DSL 求值时
  `mod.decem_icons` 必须已存在。
- **只建一次**：样式重载会重跑 `script_mod` 但 heap 不重置，用
  `OnceLock` 挡住重复的 `binary_resource`，否则每切一次深浅色多留一份字节。
- `res.binary_resource`（`res.rs:1186-1218`）直接建
  `CxScriptResourceData::Loaded`；`DrawSvg::load_svg`
  （`draw/src/shader/draw_svg.rs:466-501`）先 `cx.get_resource`，命中就不碰磁盘。
- **`include_bytes!` 仍要求编译期源码树里有这六个文件**（Rust 的语义如此）。
  也就是说：编译要源码树，**运行不要**。

验证（真实输出）：

```text
# 1) 正常构建
cargo makepad desktop build -p decem-makepad --release   → Finished release in 4.12s
grep -ac "M11.017 2.814a1 1 0 1 1.966" target/release/decem-makepad.exe → 1   # SVG 路径数据已在 exe 里

# 2) 移走 resources/icons，运行「已构建的」exe
mv resources/icons /tmp/icons-emb-bak
./target/release/decem-makepad.exe                        → 日志 [E] = 0
截图（深色 1470×1140）：sparkles / github / settings / image / send 全在；
打开设置抽屉：bot 也在。六个图标一个不少。

# 3) 目录缺失时重新构建（预期失败，说明依赖只在编译期）
cargo makepad desktop build -p decem-makepad --release
error: couldn't read `src\../resources/icons/bot.svg`: 系统找不到指定的路径。 (os error 3)
error: could not compile `decem-makepad` (bin "decem-makepad") due to 6 previous errors

# 4) 目录改回，重新构建
mv /tmp/icons-emb-bak resources/icons
cargo makepad desktop build -p decem-makepad --release   → Finished release in 4.12s
```

发布相关：

- Windows：把 exe 单独拷走也能画出六个图标（不再需要 `resources/`）。
- macOS `.app`：`desktop_bundle.rs:268-314` 仍会拷 `resources/`，但现在不是必需。
- Android：`include_bytes!` 是纯 Rust 的编译期嵌入，与平台无关；APK 里
  `cargo_makepad` 照样把 `resources/` 打进 assets（`android/compile.rs:1780-1830`），
  两条路都成立，图标不会因为这次改动失效。

### 25.3 发送键禁用态的图标着色

`draw_icon` 是单色 uniform，没有 `color_disabled` 这种状态槽位，
所以 `sync_send_enabled` 里跟着禁用态直接改色：

```rust
let mut send_button = self.ui.widget(cx, ids!(send_button));
send_button.set_disabled(cx, !enabled);
let icon_color = if enabled { rgba(0xFFFFFFFF) } else { palette().disabled_fg };
script_apply_eval!(cx, send_button, { draw_icon +: { color: #(icon_color) } });
```

两个坑：

1. 必须写 `draw_icon +:`（合并），写 `draw_icon:` 会报
   `type mismatch for property draw_icon: expected DrawSvg, got object`，
   因为那是拿一个对象整体替换 DrawSvg。
2. `script_apply_eval!` 的第一个参数要 `mut` 绑定，否则
   `cannot borrow value as mutable`。

### 25.4 在线点

网页版 `absolute -bottom-0.5 -right-0.5 w-3 h-3` → 12px、右/下各溢出头像 2px、
2px `border-background`。原来 `margin: Inset{left: 28 top: 28}` 是贴死右下角、
不溢出。改成 `left/top: 30`（40 + 2 − 12），并给 40×40 的 Overlay 容器
`clip_x/clip_y: false`，否则溢出部分被裁。颜色 `#x22C55E`、描边
`palette().bg`（深色 `#000000` / 浅色 `#F2F2F2`）、圆角 6 本来就对。

> **本节已被 §26.2 取代**：`padding 40 - 30 = 10` 会把 12px 的点压成 10px，
> 视觉上偏小。现在改成 16px + 42×42 定位层 + `align`（无 padding）。

### 25.5 输入框文字

核对后**无需改动**：`font_size: 14`、`padding: 8/10`、
`color_empty: palette().muted_fg`（深色 `#B3B3B3`、浅色 `#666666`），
与网页版 `text-sm py-2.5 px-2 placeholder:text-muted-foreground` 一致。

### 25.6 验证

- `cargo check`、`cargo clippy --release --all-targets -- -D warnings`、
  `cargo fmt`、`cargo makepad desktop build -p decem-makepad --release` 全绿。
- 深色 / 浅色各跑 7 秒后 kill，日志 `grep -ac "\[E\]"` 与
  `grep -aic "font miss"` 都是 `0`。
- 截图（kimi-cu，1470×1140）：顶栏在线点在头像右下并溢出 2px、星芒是四角星
  + 小点、GitHub 是 octocat 描边、设置是描边齿轮；输入胶囊里的图片键是描边
  图片框、发送键是纸飞机（空输入时灰、输入 `hi` 后变白 + 珊瑚底）；
  抽屉「切换模型」是小机器人。
- 已知差异：attached 图片键 hover 时网页版会把图标染成 `#F38181`，
  现在只有 hover 底色变、图标色不变（`draw_icon` 拿不到按钮的 hover 状态）。

## 26. 打字机、在线点与抽屉图标收尾（2026-10-18）

### 26.1 打字机（received / streaming 分离）

网页版 `src/hooks/useChat.ts` 是「整段收完后按 8ms/字逐字贴进气泡」。
此前本版把每个 SSE delta 直接写进显示文本，上游一快就整段蹦出。
现在 `Chat` 拆成两个字段：

| 字段 | 含义 |
| --- | --- |
| `received` | `poll_worker` 只往这里追加 delta，是上游已送到的全文 |
| `streaming` | 已经贴到气泡里的前缀，永远是 `received` 的字节前缀 |
| `upstream_done` | 上游已 Done；打字机继续把余下字符打完才收尾 |

- 节拍：`cx.start_interval(0.016)`（60Hz），每拍按
  `16ms / 8ms = 2` 字推进，UTF-8 安全（`typewriter_span` 用 `char_indices`）。
  没有待打字符时不重绘。没有为它再开 1kHz 定时器。
- 收尾：`Done` 只置 `upstream_done`，`tick_typewriter` 在
  `streaming.len() >= received.len()` 时才调 `finish_stream()`。
  `finish_stream` 只提交**已显示**的 `streaming`，所以 Esc 取消时
  上游多送的一截不会突然补出来。
- Markdown 自带的 `start_streaming_animation()`（整段排版后淡入）不再调用，
  只在流式项新建时 `stop_streaming_animation()` 清掉可能残留的状态。
- 光标：7×15、`#F38181`、圆角 1.5，`0.8s step-end`（每 0.4s 翻转）。
  用 `CURSOR_ON: AtomicBool` 在 App 定时器里写、ChatList 绘制时读。
  它是 Char 模板里的 `stream_cursor`，默认 0×0/隐藏；流式时用
  `Walk{abs_pos: Some(...)}` 放到最后一个字符右侧 2px。
  **绝对定位的 walk 不参与流式布局**，因此不影响换行与气泡量宽。
- 位置来源：先 `item_widget.draw_all_unscoped()` 画出正文，再用
  `TextFlow::area().rect(cx)` 取当前帧矩形（`Area::rect` 只在本帧重绘后有效，
  `platform/src/area.rs:403`；画之前读会得到 0，这是第一版光标不显示的原因）。
  然后用 Markdown 自己的 `draw_text` + `text_style_normal`，按 `mspace_1=3px`
  内宽重排当前文本，取最后一行 `origin_in_lpxs.x + width_in_lpxs`。
  最后单独 `cursor.draw_all_unscoped(cx)` 画在正文之上。
- 已知限制：x 用的是原始 Markdown 文本（含 `**` 等标记）的重排宽度；
  纯文本聊天准确，粗体/行内代码混排时可能有几像素偏差。y 取 TextFlow 区域底部，
  与最后一行对齐。光标只在流式中出现，结束后随流式项一起消失。

### 26.2 在线绿点改为 16×16

旧写法 `40×40` 定位层 + `padding left/top 30`，内框只剩 `10px`，
12px 的子部件被压成 10px 且没有溢出 —— 这是它看起来偏小的原因。
现在：

- 定位层 `42×42`（= 40 + 右/下各溢出 2），`flow: Down` +
  `align: Align{x:1, y:1}`，**不用 padding**，子部件不会被缩小。
- 绿点 `16×16`、2px `palette().bg` 描边、`border_radius: 8`（半宽）。
- 两层都保留 `clip_x/clip_y: false`；42×42 层从头像原点开始，右下角
  正好落在 `40 + 2` 的位置。

### 26.3 抽屉最后三个分区图标换成 lucide

新增 `resources/icons/user.svg`、`moon.svg`、`sun.svg`、`trash-2.svg`，
均取自 lucide **0.562.0**，`include_bytes!` 嵌进二进制，经
`install_icons` 注册为 `mod.decem_icons.user/moon/sun/trash`。

- 静态图标用 `Svg { width:16 height:16 animating:false draw_svg +: {...} }`。
  `Svg` **没有 `visible` 属性**（属性表里只有 width/height/abs_pos/…），
  所以 sun/moon 各包一层 16×16 的 `View` 来显隐：
  `sync_theme_ui` 只切换两个 View 的 visibility，不再写 FontAwesome 码位。
- 颜色：user `#x95E1D3`、sun/moon `#xFCE38A`、trash `#xEF4444`。

### 26.4 验证

```text
cargo check                                  → Finished dev profile
cargo clippy --release --all-targets -- -D warnings → Finished release profile
cargo fmt / cargo fmt --check                → FMT_OK
cargo makepad desktop build -p decem-makepad --release → Finished release in 4.31s
```

- 深色 / 浅色各跑 7 秒后 kill：
  `[E]` = 0、`font miss` = 0（两次独立运行都是）。
- 打字机实测（kimi-cu，1470×1140）：发送
  「用50字介绍你自己。」，0.4s 间隔连拍。0.4–1.2s 气泡是 `...`；
  1.6s 只有「我是 Decem.」；2.0s 追加到「…准备捣乱～」；随后完整。
  中途截图中最后一个字符右侧能看到 7×15 珊瑚色块光标，闪烁有亮/灭两态，
  流式结束后消失。另按任务要求对长回复在 1.5s / 3s / 6s 各截一张：
  1.5s 为 `...`、3s 为开头一小段、6s 为完整段，确认不是一次性蹦出。
- 抽屉（深色/浅色各一张）：user 是描边人形、浅色模式显示太阳、
  深色模式显示月亮、清空分区是描边垃圾桶；在线点明显比旧的 12px 大，
  位于头像右下并溢出 2px。

## 27. 输入框多行、按钮圆角、外链与聊天记录持久化（2026-10-19）

### 27.1 Shift+Enter 换行：`is_multiline` + `submit_on_enter`

之前输入框只设了 `width/height/padding`，`TextInput` 默认 `is_multiline: false`，
所有 Enter（含 Shift+Enter）都会直接发 `Returned`，Shift 被吞掉。

```text
input := glass.TextInput {
    height: Fit{min: FitBound.Abs(40), max: FitBound.Abs(120)}
    is_multiline: true
    submit_on_enter: true
    keep_focus_on_submit: true
    ...
}
```

- `submit_on_enter: true`：物理键盘上**裸 Enter 发送、Shift+Enter 插换行**
  （`widgets/core/src/text_input.rs:3229-3300`）。`is_multiline: false` 时
  Shift+Enter 分支会走 `submit()`，所以两者必须一起开。
- `keep_focus_on_submit: true`：发完不清键盘焦点（网页版 textarea 也保持焦点）。
- 高度 `Fit{min:40, max:120}` 对应网页版 `min-h-[40px] max-h-[120px]`：
  单行与 40px 按钮同高，多行自动长高，到顶后输入框内部滚动。
- 行容器与胶囊都改成 `height: Fit` + `align{y: 1.0}`（网页版 `items-end`）。
  **不要**在 Fit 高度下用 `flow: Overlay` 叠一圈 `height: Fill` 的描边层——
  Fill 子部件拿不到父高。聚焦环改为直接切 `pill_bg` 的
  `draw_bg.border_size`（`script_apply_eval!`，0 / 2px），仍然是内描边、不占布局。

### 27.2 Button 的 SDF 圆角是「视觉半径的一半」

`Sdf2d.box/box_all` 内部是 `k = 2 * r`（`draw/src/shader/sdf.rs:410-430`），
所以 `ButtonFlat` / `glass.Button`（以及所有走 `sdf.box` 的按钮）写
`border_radius: 12` 实际画出来是 **24px 圆角**，40×40 的按钮就变成正圆。

| 想要 | 网页 class / 值 | 按钮里写 |
|---|---|---|
| 16px | `rounded-xl`（本项目 `--radius:12px` + 4） | `8.0` |
| 12px | `rounded-lg` | `6.0` |
| 10px | `rounded-md`（`--radius - 2`） | `5.0` |

`RoundedView` / `View` 的 `draw_bg` 是另一套 shader，`border_radius` 就是字面值，
**不要**一起减半。胶囊 16、抽屉 0、开关轨道 12 都是字面值。

### 27.3 Windows 的 `Cx::open_url` 是空实现

`platform/src/os/windows/windows.rs:1474` 只 `error!("open_url not implemented")`，
所以 GitHub 图标点了没反应。桌面端自己调 `ShellExecuteW`
（`shell32`，`open` + `SW_SHOWNORMAL`）；macOS / Android 后端有实现，
继续走 `Cx::open_url`。见 `main.rs::open_external_url`。

### 27.4 聊天记录持久化

`%APPDATA%\DecemChat\chat-history.json`（与头像、主题同目录）：

- 只存 `role/text/time`，最多最近 50 条——与网页版 `useChat` 的 localStorage
  策略一致；**图片 base64 不落盘**，重启后图片不恢复（网页版也只把图放 IndexedDB）。
- `save_history` 在四个点落盘：用户消息入列、流式收尾（含 Esc / 失败）、清空、
  重新生成截断。不在每个流式 delta 上写盘。
- 启动时 `load_history` 恢复；文件不存在是正常情况，格式坏了会追一条 `⚠️`
  消息（不静默吞掉）。

### 27.5 抽屉细节对齐

- 右上角 ✕ 从文本「×」换成 lucide `x`（`resources/icons/x.svg`，
  与其余图标同一套 `include_bytes!` + `mod.decem_icons.x`）：
  36×36 按钮、20px 图标、视觉圆角 12（按钮写 6）。
- 头部 `p-4` → 高度 68（36 按钮 + 上下 16），不再是固定 56。
- 段内间距 8 → **12**（网页版 `space-y-3`）。
- 开关 32×18.4/16 → **44×24/20**（`switch_knob_left()` 同步改成 44-20-2）。
- 清空区标题颜色改 `#EF4444`（网页版 `text-destructive`）。

### 27.6 验证

```text
cargo check / cargo clippy --release --all-targets -- -D warnings → 0 warning
cargo makepad desktop build -p decem-makepad --release            → Finished
深色 / 浅色启动                                                   → [E]=0, font miss=0
```

kimi-cu 实测（1470×1140）：

- 输入「你好」→ Shift+Enter →「第二行」：输入框长到两行，发送后上游回复
  正确引用「第二行」，确认换行保留。
- 重启 exe：`chat-history.json` 里的三条消息完整回到列表。
- 点 GitHub 图标：Edge 新标签打开 `defaultuser233/DecemChat`。
- 点图片按钮：弹出「选一张图」文件对话框；`DECEM_IMAGE` 注入时占位文字
  切换为「添加描述（可选）...」，发送键点亮。
- 已知限制（**已于 §29 修正**）：这一版抽屉的 glass 底还是纯色 0.8；后来发现
  Makepad 自带 `GaussRoundedView` 能采样并模糊窗口背景，见 §29。

## 28. 按 CSS/JS 源码做像素级对齐（2026-10-19 第二轮）

这一轮不再按截图猜，直接对着 `src/index.css`、`tailwind.config.js`、`src/components/*.tsx`
和 `ui/button|switch|select|alert-dialog.tsx` 逐项改。下面是容易再踩的结论。

### 28.1 `RoundedView` / `Image` 的圆角也是「视觉半径的一半」

上一节只写了 Button 的 SDF；实测 `mod.widgets.RoundedView`（`view_ui.rs:351-390`）
和 `Image`（`image.rs:217-221`）同样走 `Sdf2d.box`，内部是 `k = 2 * r`
（`draw/src/shader/sdf.rs:410-430`）。**所有 RoundedView/Image 的
`border_radius` 都要写 CSS 值的一半**，否则会圆一倍：

| CSS | 写入 |
|---|---|
| 胶囊 `rounded-2xl` 16 | 8 |
| 气泡 18 / 4 | 9 / 2 |
| 代码块 `rounded-lg` 12 | 6 |
| 主题行 `rounded-lg` 12 | 6 |
| 触发器/列表/清空 `rounded-md` 10 | 5 |
| 模型行 `rounded-sm` 8 | 4 |
| 图片预览 `rounded-lg` 12 | 6 |
| 消息图片 `rounded-lg` 12 | 6 |
| 圆/胶囊（头像、开关、徽章） | 半边长/2 即可饱和 |

判断依据：改之前 56px 高的输入胶囊写 16，渲染出来是 32 → 两端全圆，和网页版
16px 圆角矩形明显不同。

### 28.2 气泡小角方向：以 CSS 简写为准

`border-radius: a b c d` 的顺序是 **TL TR BR BL**：

- 用户 `.message-bubble-user: 18px 18px 4px 18px` → 小角在**右下**（朝向右侧用户头像）
- char `.message-bubble-bot: 18px 18px 18px 4px` → 小角在**左下**（朝向左侧 char 头像）

`docs/makepad-visual-spec.md` 之前写反了，已修正。每个角用
`RoundedAllView`（不是 `RoundedView`）的 `border_radius: uniform(vec4(...))`，
顺序是 **lt, rt, rb, lb**。

### 28.3 中文字体：系统雅黑，不是 LXGW

网页字体栈在 Windows 上把中文落到 **Microsoft YaHei**；Makepad 自带的
CJK 兜底是 LXGW WenKai（楷体），观感差别很大。现在用
`mod.res.file_resource("C:/Windows/Fonts/msyh.ttc")`（粗体 `msyhbd.ttc`）
接进 `font_regular/font_bold/font_code/font_display` 的 CJK 槽位，
非 Windows 才退回 LXGW。`file_resource` 是运行期按绝对路径读，不把
36MB 字体塞进 exe；`ttf_parser` 能解析 TTC 的 face 0。

### 28.4 消息区尺寸（源码值）

- 滚动容器 `px-4 py-4`；相邻消息 `space-y-6`。实现：每条消息上下 padding 12，
  容器上下 padding 4（首尾合计 16，之间 24）。
- 消息列 `max-w-3xl` = 768；内容列 `max-w-[75%]` = 576 → `Bubble.cap_width = 576`。
- 气泡 `px-4 py-3` = 左右 16、上下 12 → `pad_x = 32`。
- 正文 `text-sm leading-relaxed` = 14px / 1.625em。Makepad 的 `line_spacing`
  是自然行高的倍数（Lato 约 1.2em），写 **1.35** ≈ 22.7px。
- Markdown：`paragraph_spacing: 4`（网页 `p mb-1`）、
  `heading_base_scale: 1.6`、行内代码 12px/内距 6×2/2、`code_color #00000033`
  （网页 `bg-black/20`）、代码块底 `#0000004D`（`bg-black/30`）。
- Markdown 自带的 `padding: theme.mspace_1`（3px）会让正文比气泡多缩进，
  消息里的 Markdown 显式写 `padding: 0`。
- 时间戳：`mt-1` 4、`gap-2` 8、12px muted；重新生成 12px 珊瑚 +
  lucide `rotate-ccw` 12（不再是 `↻` 文本）。

### 28.5 抽屉/下拉（源码值）

- 头部 `p-4`：36 关闭键 + 上下 16 = 68 高；关闭键 20px lucide `x`、圆角 12。
- `space-y-6` 24；每段 `space-y-3` 12。
- 开关按 `ui/switch.tsx`：`h-[1.15rem] w-8` = 18.4×32、滑块 `size-4` 16，
  选中位移 `calc(100%-2px)` = 14。**不要**按 Tailwind 文档默认的 44×24。
- 上传/清空按钮 `size=sm`：h-8 32、rounded-md 10、text-sm 14、图标 16、gap 6；
  描边用 `border-input`（#262626 / #E6E6E6），底 `bg-input/30`。
- 模型下拉是 **Portal 浮层**：新建 `model_dropdown := Modal`，透明遮罩、
  右对齐、距顶 167，宽 320（内距 16）；不再参与抽屉布局，也不再把下面的
  分区往下推。行高 `Fit`（约 50）、`py-1.5 pr-8 pl-2`、选中/悬停
  accent `#8FD4E5` + 黑字，右侧 lucide `check` 16。
- 清空确认框按 `alert-dialog.tsx` + `glass-effect`：max-w-lg 512、
  rounded-lg 12、p-6、gap-4、玻璃底；标题 18、描述 14。

### 28.6 图片消息与持久化

- 预览：网页 `h-20` 自动宽。解码后 `photo.size_in_pixels(cx)` 取真实尺寸，
  `set_walk_and_fit` 设成 `80 × (80*w/h)`（上限 240），圆角 6、1px 边框、下边距 12。
- 消息图：`max-h-48` 192、不放大（scale ≤ 1），圆角 6、下边距 8；
  气泡宽度 = `max(文字宽度, 图片宽度) + 32`（`BUBBLE_BODY` 的第三项传图片宽）。
- 持久化升级：图片字节写到 `%APPDATA%\DecemChat\images\<content-hash>.<ext>`，
  JSON 只存文件名（对应网页版的 IndexedDB）；重启后按文件名读回并重建 data URL。
  旧 JSON 里没有 `image` 且文字为空的消息直接丢掉（否则会出现空通栏气泡）。

### 28.7 其他

- 滚动条照 `.custom-scrollbar`：6px、透明轨道、珊瑚 30%（hover 50%）、
  `auto_hide`。用户侧消息右侧留 8，让滚动条不压头像。
- 背景加了一层 ±1.2% 的逐像素灰噪声，近似 `.noise-overlay` 的 3% fractal noise
  （`Math.random_2d`；`clamp` 在脚本里必须三个参数同型，用 `vec3` 版本）。
- 仍做不到：`backdrop-filter: blur()`（抽屉/顶栏/输入区）、`font-medium` 500
  （Lato 没有 500，浏览器也是合成）、`letter-spacing`（TextStyle 无此字段）、
  头像 scale 呼吸/hover 旋转与消息 slide-up（View 没有组透明度和变换）。

## 29. 更正 §28.7 的「做不到」清单（源码核实）

§28.7 有两条结论是凭印象写的，源码里其实有现成机制。以下都给出文件与行号。

### 29.1 背景模糊：`GaussRoundedView` 能做，已接进抽屉

`widgets/core/src/gauss_view.rs:1-40` 的模块注释写得很清楚：它会采样窗口
自己的画面，渲染到纹理并生成 mip 金字塔，再通过圆角 SDF 折射/模糊。
`blur_level` 是 **mip 层级**（0=原图，6=最糊），不是像素。`tint_color` +
`tint_alpha` 就是玻璃的底色（`tint_alpha 0.8` = 网页版 rgba(...,0.8)）。

抽屉面板已从 `RoundedView` 换成：

```text
drawer_panel := GaussRoundedView {
    draw_bg +: {
        blur_level: 3.0
        edge_blur_level: 3.0
        tint_color: palette().glass
        tint_alpha: 0.8
        surface_alpha: 1.0
        lensing_effect: 0.0
        rim_alpha: 0.0
        inner_shadow_alpha: 0.0
        specular_strength: 0.0
        corner_radius: 0.0
    }
}
```

它放在 Modal 的 overlay 里，按模块注释会「bind the window's capture and draw
inline」，不会另开一层。实测抽屉背后是模糊过的消息/图片，不再是清晰轮廓。

还没做的：遮罩的 `backdrop-filter: blur(4px)`。`Modal` 的 `bg_view` 是一个
固定 View，要整屏模糊得把 Modal 内容改成 Overlay 并把 bg_view 透明，
再自己放一层全屏 Gauss；点击关闭的命中逻辑要一起改，风险高于收益。

### 29.2 旋转：`DrawSvg.svg_rotation` / `Image.rotation` 都有

- `draw/src/shader/draw_svg.rs:33-35`：`svg_rotation` 是 DrawSvg 的 uniform，
  单位弧度（绕 rect 中心）。设置齿轮的 hover 直接给 Button 的 animator
  增加了 `draw_icon.svg_rotation: 0 → π/2`、0.3s 的过渡（Button 原生
  hover/down 状态机，`widgets/core/src/button.rs:370-402`），底色混合保持不变。
- `widgets/core/src/image.rs:28-33, 258-268`：`Image` 有 `rotation`（角度）和
  `image_scale`（vec2）两个 live 字段，`ImageRef::set_uniform` 可以在运行时
  直接推。所以头像 hover 的 `rotate(5°)` 和呼吸的 `scale(1→1.02)` 不是
  「做不到」，只是需要一个 hover 命中与 10Hz/60Hz 的相位更新；这轮先没接。

### 29.3 消息 slide-up + 淡入：可以，但要自绘快照

`View` 没有组透明度，但有 `texture_caching` 和
`View::take_texture_snapshot(cx)`（`widgets/core/src/view.rs` 的
`ViewTextureCache` / `ViewTextureSnapshot`）。`apps/calendar/src/navigation.rs:90-218`
就是拿它实现翻页：把旧的子树冻结成纹理，再用一个带 `opacity` uniform 的
`DrawQuad` 在偏移后的 rect 上画出来，同时画新页。

所以 `animate-slide-up` 的正确做法是：给消息项加一个薄包装部件，新项出现时
把子树画进缓存纹理，之后 0.5s 内用 `opacity 0→1` + `y +20→0` 画那张纹理，
结束后切回实时子树。工程量在「包装部件 + 动画状态」而不是引擎能力。

### 29.4 字号 500 / 字距：这两条确实是死路

- `TextStyle` 只有 `font_family / font_size / line_spacing / top_drop`
  （`draw/src/shader/draw_text.rs:3534-3543`），没有字距。
- 更底层 `ShapeParams` 有 `letter_spacing`（`draw/src/text/shaper.rs:249-255, 499`），
  但 `FontFamily::get_or_shape` 把它**写死成 0**（`draw/src/text/font_family.rs:70-82`），
  没有任何 live 字段能传进去。要支持得改 Makepad 依赖。
- `FontMember.weight` 会映射到 OpenType `wght` 轴；Lato 是静态字体、没有这个轴，
  所以 500 没有独立字形。浏览器对静态字体也是合成加粗，不是真 500。要严格
  一致得换可变字体（会改字形，不只是字重）。

## 30. 用户反馈七项修复（2026-10-20）

### 30.1 绿点放大到 16

网页 CSS 是 `w-3`（12px），但用户两次反馈偏小；按用户要求改为 **16×16**
（定位层仍 42×42，右下各溢出 2），2px 背景色描边不变。

### 30.2 输入区按钮垂直居中

之前照网页 `items-end` 写 `align{y:1.0}`，单行时输入框自然高度略大于 40
会让按钮偏下。按用户要求把外层行和胶囊内行都改成 `align{y:0.5}`。

### 30.3 按钮 hover：DrawSvg 的颜色不能直接改

实测 `button.draw_icon.color = coral` 之后日志确认字段已变、渲染仍是白色。
原因：DrawSvg 的 `color` 是 shader 里的实例色，Rust 字段只是镜像；而
`script_apply_eval!` 到 Button 上会重新 apply 整个对象、丢掉已加载的 SVG
（`widgets/core/src/button.rs:1152-1155` 的注释也这么说）。

可靠做法：**叠两个静态 Svg**（正文色 / 珊瑚色），用 `set_visible` 切显隐；
透明 ButtonFlat 压在最上层负责命中与 hover 底色。Rust 侧在 `Event::MouseMove`
里用 `event.hits(cx, button.area())` 判 hover，切换两个 View。
图片按钮的 hover 底色也从 10% 提到 15%（`#F3818126`），状态更明显。

### 30.4 GIF 抽帧（阿里云读不了动图）

新增依赖：`image`（gif/jpeg/png），`fontdue`（画帧号）。

流程照 `src/services/gifFrames.ts`：

1. `image::codecs::gif::GifDecoder` 解出**已按 disposal 合成**的全画幅帧
   （image 的 GifFrameIterator 内部会 `blend_and_dispose_pixel`）。
2. 按累计时长等间隔取至多 **6 帧**（`pick_gif_indices`，与 JS 同算法）。
3. 每帧铺白底、最长边缩到 **512**；网格 1-3 帧单行、4 帧 2×2、5-6 帧 3×N，
   间距 6、底部标签带 20；用 fontdue + 内嵌 Lato 12px 画帧号 1..6（灰色 #666）。
4. **输出 JPEG 82 而不是 PNG**：4 帧 512 的 PNG 达 1.7MB，上游直接 413；
   JPEG 通常 300–500KB。预览、消息气泡和持久化都用这张网格。
5. `hint` 通过 `decem_core::Message.hint` 只发给上游（不进气泡）：
   `[此图为GIF动图抽取的N帧拼接，按时间先后顺序排列]`，并随聊天记录持久化。

### 30.5 请求体预算：900KB，从最旧的消息丢图

上游 nginx 的 `client_max_body_size` 默认是 1MB（服务端 express 的 20mb
在它后面）。实测带 3MB 图片历史会 413。`build_request_body` 现在循环：
超过 900KB 就把 `keep_from` 往前挪，**从最旧的消息开始不带图**；连最新
一张图都丢掉还超限才报错（提示换小图），不会静默发一条没有图的请求。

非 GIF 图片在选图时也会压缩（网页版 `images.ts` 同款）：
>300KB 才处理、最长边 1568、PNG 保持 PNG、其余 JPEG 82。

聊天记录改为**临时文件 + rename** 原子写入，避免进程在写盘中途被杀导致
JSON 半截（实测并发读时见过一次半截文件）。

### 30.6 模型下拉的关闭

之前下拉的 `Modal` content 是 320 宽全高，点抽屉里列表以外的区域仍算
content、不会 dismiss，用户找不到关闭方式。现在 content 铺满窗口：
底层一个全透明 `ButtonFlat`（`dismiss_models_button`）接收列表外的点击，
列表在它上面；点任何地方都能关。Esc/选模型仍由 Modal 自己处理。

### 30.7 滚动条移到窗口右缘

原来是 `ChatList { max_width: 768 }` 内嵌滚动，滚动条落在 768 列右边缘、
贴着用户头像。现在：

- `ChatList` 铺满窗口（PortalList 的滚动条在窗口右缘）；
- 每个 item 内部套「全宽 wrapper（align x:0.5）→ max_width 768 的内容列」；
- item 的左右 padding 移到 wrapper（16），上下仍在内容列（12）+ 列表 4。

### 30.8 思考动画

首帧（`streaming.is_empty()`）显示用户给的 **`loading-27`** 同款：
三个同心椭圆环——外环 2s 一圈、内环 3s 正转、另一个 3s 反转，
右侧是「狐狐正在思考...」14px。

- 椭圆环用 shader 的隐式曲线 `(x/a)²+(y/b)²-1=0` 画，环厚按梯度归一化
  距离计算，任意旋转角度下都是平滑矢量边，没有位图锯齿；
- CSS 的 60×25 按比例缩到 **40×40** 容器（椭圆半轴 19×8，环厚 1.5），
  旋转椭圆的完整包络正好放进这个盒子；
- `draw_pass.time` 驱动三个角度（`t·π`、`t·2π/3`、`-t·2π/3`），
  可见时窗口自动逐帧重绘；隐藏后不再占帧；
- 之前的双心/emoji 动画已移除（`theme.font_emoji` 也删掉了）。

### 30.9 验证

```text
cargo clippy --release --all-targets -- -D warnings → 0 warning
cargo makepad desktop build -p decem-makepad --release → Finished
深色 / 浅色启动 → [E]=0，font miss=0
```

- GIF（gifuct-js 的 dog.gif）：预览/气泡为 2×2 网格；模型正确描述狗的四个动作。
- 首次 PNG 网格 413，改 JPEG + 900KB 预算后同一条历史发送成功。
- hover 图片按钮：图标变珊瑚、底色变亮（两张截图对比）。
- 模型下拉：点抽屉任意非列表位置即关闭。
- 思考态：截图可见三个珊瑚椭圆环（2s/3s/反向 3s）+「狐狐正在思考...」。
- 图片压缩：>300KB 的 JPEG 会重编码到最长边 1568；GIF 网格不受影响。

## 31. 第二轮反馈四项（2026-10-20）

### 31.1 纸飞机不居中：空 label 仍占 spacing

Button 的图标和文字之间会保留 `spacing`（默认 `theme.space_2` = 6）。
纯图标按钮的 label 宽度是 0，但 6px 间距还在，图标被整体左移 3px。
发送键、GitHub、设置、抽屉关闭键都加 `spacing: 0`，纸飞机回到正中。

### 31.2 「Ave atque vale」的彩色/动效

之前 `#(footer_glow())` 是**脚本求值一次**就定格的，10Hz 重绘不会重新求值，
所以看起来是静态单色。现在：

- 把文本拆成三个 Label（`Ave` / `atque` / `vale`），各自取渐变相位
  0 / 0.33 / 0.66（`footer_color(offset)`）；
- 光晕定时器每 100ms 用 `set_text_color` 把三色推进一格；
- 视觉上是三色词同时存在、并沿珊瑚→薄荷→黄缓慢流动。

真正的 `background-clip: text` 逐字渐变仍然做不到（TextStyle 无渐变；
自定义 DrawText shader 需要改 Makepad 依赖）。

### 31.3 模型列表黑底黑字：Fill 高亮在 Fit 父级里塌成 0

上一轮为了贴合网页行高把 `ModelRow` 改成 `height: Fit`，但 `row_hl`
是 `height: Fill` 的叠层——Overlay + Fit 父级拿不到高度，高亮层消失；
选中/悬停行把文字设成黑色，就变成黑底黑字。
行高改回固定 **52**（两行 14/12 + 上下 6 + 2 间距 ≈ 50，接近网页行高），
`row_hl` 恢复 Fill 高度，选中行重新是薄荷底 + 黑字，其余行白字。

### 31.4 顶栏头像用错了

`header_avatar_bytes` 之前优先返回用户上传的头像；网页版顶栏始终画
`settings.charAvatar`（Decem），「更改头像」只影响用户消息一侧。
现在固定返回 `avatar_bytes(Role::Char)`，并删掉 `UserAvatar.custom`
（它只为顶栏存在）。用户头像仍用于消息列表和设置预览。

## 32. 气泡右侧空一块（2026-10-20）

用户截图里「咪的天」右侧明显多出一截。根因是 `Bubble` 量宽时**无条件**
加了 16px 余量：

```text
width = 文本宽度 + 左右 padding(32) + 16
```

这 16px 是当初为了掩盖「量宽实例与 Markdown 渲染实例的 emoji/字距差异」
加的，防止欢迎语提前折行；但短消息没有折行问题，结果就是右边比左边多
16px 空白。

现在只在两种情况下保留 16px：

- 文本含 emoji（`has_emoji`：U+1F000–1FAFF、U+FE0F、U+2600–27BF）；
- 文本宽度已接近换行上限（`used + pad_x + 24 >= cap`），不给余量会提前折行。

其余短消息只留 **2px** 舍入余量。实测「咪的天」气泡左右各 16px，
长消息的换行点与之前一致。

## 34. 苹果式圆角矩形（squircle，2026-10-20）

用户要求把 UI 的圆角矩形改成苹果那种连续圆角。实现分两层：

### 34.1 `SquircleView`

新增自定义 `SquircleView`（`View` + 自己的 `draw_bg.pixel`），不是普通
rounded-rect 的「四分之一圆弧 + 直线」，而是**四次超椭圆**：

```text
q = abs(p - half) - (half - r)
d = (max(q.x,0)^4 + max(q.y,0)^4)^(1/4) + min(max(q.x,q.y),0) - r
```

四次超椭圆在圆角与直边的衔接处曲率连续，就是 iOS 图标/Continuous Corners
的观感；普通圆弧在衔接处曲率突变。`corner_n` 可调（默认 4.0）。

- `border_radius` 是 **vec4 视觉半径**（TL/TR/BR/BL），不再像 `RoundedView`
  那样写「视觉的一半」；
- 支持 `color` / `color_2` + `gradient_fill_horizontal`（用户气泡的珊瑚渐变）；
- 支持 `border_size` / `border_color`：外圈用边框色填满，再内缩 `border_size`
  填内容色，得到标准的 1px/2px 内描边；
- `Sdf2d.shape` 直接写超椭圆距离，复用 Makepad 的 `fill_keep` 抗锯齿。

已替换：输入胶囊（16）、char 气泡（4/18/18/18，左上小角指向头像）、
用户气泡（18/4/18/18，右上小角指向头像）、
代码块（12）、模型触发器（10）、模型列表（10）、模型行高亮（8）、
深浅色行（12）、清空确认卡片（12）。

### 34.2 `SquircleButton` / `SquircleFlatButton`

Button 的 `draw_bg` 是自定义 shader，直接换 widget 会丢 hover/down 混色。
做法是按 `glass.Button` / `ButtonFlat` 派生一个模板，只覆盖 `pixel`，
继续调用继承来的 `face_fill()` / `face_stroke()`：

- `border_radius` 仍是按钮模板的「视觉半径的一半」，shader 里 `* 2`；
- 外圈 `face_stroke()`、内圈 `face_fill()`，保留 disabled/hover 的混色；
- 已替换：发送、图片、GitHub、设置、抽屉关闭、上传头像、清空记录、
  确认框的取消/确认、模型触发器覆盖层（主题开关的透明命中层也换了，
  但它是圆形轨道上的不可见层，不影响外观）。

### 34.3 仍保持原形的部件

头像（圆形）、在线点（圆形）、开关（胶囊）、滚动条（细长圆角条）、
消息图片/预览图（Image 自己的圆角裁剪，不是 UI 面）。这些本来就不是
「圆角矩形」，不需要 squircle。

### 34.4 验证

```text
cargo clippy --release --all-targets -- -D warnings → 0 warning
cargo makepad desktop build -p decem-makepad --release → Finished
深色 / 浅色启动 → [E]=0，font miss=0
```

深色/浅色截图确认：输入胶囊、两种气泡、抽屉里的触发器、深浅色行、
上传/清空按钮、确认卡片都是连续圆角；按钮的 hover/disabled 状态仍正常。

## 35. 输入框聚焦时整块变红（2026-10-20）

Squircle 的边框绘制顺序踩了半透明填充的坑：先 `fill_keep(border_color)` 铺满
外框、再内缩铺 `color`。输入胶囊的 `row_bg` 是 **50% 透明**，底下的珊瑚边框色
透上来，整个胶囊变成暗红色（用户说的“有点吓人”）。

正确顺序：

1. 先用内容色填外框（半透明也没关系，下面是输入区的深色底）；
2. 再用 `shape = max(outer, -inner)` 只填**边框环**，叠上边框色。

`outer` 在外框内为负，`-inner` 在内框内为正，所以 `max` 只在「外框内、内框外」
为负——正好是 2px 环。`SquircleView` 与两个 `SquircleButton` 着色器都按这个
顺序改了；输入框聚焦恢复为「深色胶囊 + 2px 珊瑚描边」，上传/触发器按钮的
半透明底也不再被边框色染色。

最终按用户澄清处理：**只有边框环上色，内部保持内容填充不变**。
`sync_input_focus` 在聚焦时把 `pill_bg.border_size` 设为 2px，失焦设回 0；
SquircleView 先填 `draw_bg.color`（深色半透明），再在**内缩 border/2 的轮廓**上
`sdf.stroke(border_color, border)`——带宽从外轮廓到 border 像素内侧，全部落在
圆角形状里，四边对称且角是圆的。
（踩过两次：`max(outer, -inner)` 填环只出右下两条；直接 stroke 外轮廓会把
外半圈画到四角外面，看起来是尖角。）
气泡、卡片、上传按钮等需要边框的部件共用同一套 stroke 绘制。

## 36. 齿轮与抽屉关闭键对齐（2026-10-20）

用户反馈：顶栏的设置齿轮和设置抽屉右上角的 ✕ 不在同一个位置。

根因是两层叠加：

1. `Modal::draw_walk` 用 `begin_root_turtle_for_pass` 把内容挂到**窗口 pass 的
   原点**，而 Makepad 在 Windows 上的自绘标题栏也在同一个 pass 里（`body` 才在
   标题栏下方）。抽屉于是从窗口最顶端开始，头部整体比顶栏高一个标题栏
   （150% DPI 下 44px），✕ 中心比齿轮中心正好高这么多；抽屉也会盖住标题栏。
2. 关闭键此前是 36×36、头部内距 16，和顶栏按钮的 40×40 / 内距 12 不是一套几何，
   就算起点对齐，中心也差 2px。

修法：

- 顶栏的 `RoundedView` 加 id `app_header`。Rust 侧新增 `sync_drawer_offset`，
  量 `app_header` 的绝对 `top`（= 标题栏高度；没有标题栏的平台测得 0），
  写进 `settings_drawer.content` 的 `padding.top`。抽屉从标题栏下方开始，
  标题行与顶栏同排。
- 抽屉头部改用顶栏同一套几何：高 64（40 + 上下 12）、左右 16；关闭键
  40×40、`border_radius: 8.0`（SDF 视觉半径 2×8 = 16，与齿轮一致）。
  这样两者中心都落在「右缘 −36，顶栏内容中线」。
- 偏移不能写死：标题栏高度随平台 / DPI / 最大化变化。也不能只应用一次：
  切主题走 `ScriptReapply` 会把 `#[layout]` 重置成 DSL 里的 0，所以每帧拿
  当前 `padding.top` 和实测值比，不一致就重写（缓存「已应用」标记会在换主题后
  又把抽屉缩回标题栏底下）。

验证（150% DPI）：

| 窗口 | 齿轮亮像素中心 | 抽屉 ✕ 亮像素中心 |
| --- | --- | --- |
| 1470×1140（深色） | (1415.5, 91.0) | (1415.5, 91.0) |
| 1470×1140（浅色） | (1415.5, 91.0) | (1415.5, 91.0) |
| 2560×1600 最大化（深色） | (1534.4, 55.5) | (1534.3, 55.7) |

`cargo clippy --release --all-targets -- -D warnings` 0 warning；
深浅色、最大化切换后 `[E] = 0`。
