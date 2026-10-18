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

**清空**：输入行最左的「清空」→ 内置 `Modal` 二次确认（取消 / 确认清空；
遮罩与 Esc 由 Modal 的 `can_dismiss` 免费提供）→ 先 `worker.cancel()` 再清 messages、只留欢迎语。

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
   `Image` 仍然不能进 `flow: Right`（§12 的 NaN），「🖼」按钮继续放在胶囊外。

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

改用主题里已有的 FontAwesome Solid（`theme.font_icons`，
`widgets/core/src/theme_desktop_dark.rs:677-683`）：单色字形、跟随 `color` / `color_hover`。

| 位置 | 原来 | 现在 |
| --- | --- | --- |
| 顶栏设置按钮 | `⚙︎` emoji（蓝，染不上色） | `\u{f013}` fa-gear，18px，`palette().muted_fg`，hover 变 `palette().fg` |
| 抽屉「当前」徽章 / 模型行徽章 | `👁` emoji | `\u{f06e}` fa-eye，11px，`#x4DB6AC` |

顶栏 `✨` 留 emoji：FA 免费集里没有 sparkles 字形，最近的 `fa-star`(f005)
会换成另一个图标的语义，不动。抽屉里剩下的 `🔀 👤 🌙` 同理，后续要换时照这张表加。

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
