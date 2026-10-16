# Makepad 迁移：状态、计划与作业手册

> 更新：2026-10-16　关联文档：`docs/makepad-probe.md`（探针实测记录与候选筛选）
> 这份文档的目的：**换一个会话、隔几周回来，也能立刻接着干**。

## 0. 一句话状态

方向已定：**用 Makepad（纯 Rust 自绘）重做 Decem Chat**，分四阶段、每阶段有验收门槛。
当前进度：**阶段 1 已通过验收**（`decem-makepad/`）。骨架能跑、中文输入法能上屏、流式打字机与
300 条长列表滚动都实测过了，证据见 §3。**下一道门是阶段 2：接真实网络（HTTPS + SSE）。**
现有实现（网页版 / Tauri 版 / egui 版）**全部保留、一行未动**，继续是主力。

历史坑：已排除的两个「看起来像 bug」的现象——代码块/表格空白（Markdown 宽度，见 §6.2）、
滚动没反应（测试脚本把多次滚轮挤在紧循环里，不是 app 的问题，见 §3）。

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

### 阶段 1：聊天骨架（进行中）
- **目标**：消息列表 + 输入 + 发送 + 流式打字机，数据来自**进程内模拟上游**（先不接网络）
- **为什么先不接网络**：这一步证伪的是 Makepad 本身；网络是下一道独立的门，混在一起失败时无法归因
- **验收（要人亲自跑）**：① 中文输入法能敲字 ② 回车/点按钮发送 ③ 回复逐字出现 ④ 长文本换行、列表滚动 ⑤ 全角标点与 emoji 正常
- ⚠️ **第 ① 条不过就停在这里**（中文聊天 App 没法用），届时如实报告而不是硬撑

### 阶段 2：接真实网络（第二道门）
工作线程里用 `makepad_platform::network::blocking_http`（**要求 HTTPS**，可边收边写给 writer）
→ 自己解 SSE（`crates/decem-core/src/sse.rs` 有现成解析器与测试）
→ channel + `SignalToUI` 推给 UI → 交给 Markdown 的流式动画。

### 阶段 3：功能对齐（长尾）
头像与图片消息（base64 解码 + Makepad 的 `Image`）、重新生成、清空记录（二次确认）、
模型切换面板与「视觉理解」标注、设置（API Key / 接口地址）、本地持久化、主题、Toast、欢迎语、打字速度……

### 阶段 4：全平台出包
Android 已有完整流程（见 §4）；Windows 桌面打包**待核实**（官方无工具，社区 `cargo-packager`）；
macOS/Linux 无机器可验（可 CI 编译）；iOS 需 Mac + Xcode。

## 3. 阶段 1 验收结果（已通过）

代码：`decem-makepad/src/main.rs`（约 300 行）

- `PortalList` 消息列表（左右两种气泡模板，`Char` 左/深色，`User` 右/橙 `#xE95620`）
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
| 代码块 + 表格 | ✅（回退路径） | 见 §6.2；等宽文本渲染，无语法高亮 |
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
2. **气泡与 Markdown 都必须是 `width: Fill`** —— 阶段 1 最大的坑，完整分析见 §6.2
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

### 6.2 宽度模型：为什么「贴文字」和「正常换行」不能兼得

这一节是阶段 1 踩得最久的地方，三个事实叠在一起：

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

所以**行必须有确定宽度**。行是 `Fill`、外层表是 `Fit` 时行宽为 0 → 每个格子 0 宽 →
整张表只剩高度。实测把行改成固定值就能正常渲染（列也对齐）：

```
table_row_walk: Walk{width: Fixed(560) height: Fit}     # 验证过能渲染
```

但那是写死的像素宽度，窄窗口同样溢出。

**结论**：Makepad 的 `Fit`/`Fill` 是**单趟解析**，没有 CSS 那种 intrinsic sizing
（先量 max-content、再定父宽、再让子元素 `width:100%` 回填）。所以下面三件事两两互斥，
不存在一个配置能同时拿到：

| 想要 | `Fit` 气泡 | `Fill` 气泡 |
| --- | --- | --- |
| 短消息贴着文字 | ✅ | ❌ 通栏 |
| 长文本换行 / 窄窗口不溢出 | ❌ 不换行会溢出 | ✅ |
| 代码块 | ✅（须用 `CodeBlock`） | ✅ |
| 表格 | ❌ 除非把行宽写死 | ✅ |

**当前采用 `Fill` 气泡**（`width: Fill` 的气泡 + `width: Fill` 的 Markdown，即 aichat 的写法），
理由是「不溢出」的优先级高于「好看」——聊天 App 要接的是模型任意长度的输出，
而且最终要上手机。代价就是短消息也占满一行。

真要「贴文字」，只能靠 Rust 侧按内容挑模板（短且单行、无表格 → `Fit`；其余 → `Fill`），
并且必须加"窗口太窄就退回 `Fill`"的保护。这是阶段 3 的活，动之前先把上面三条记住。

## 7. 未决 / 只有人能验的

| 项 | 谁 |
| --- | --- |
| ~~中文输入法（IME）能否在 Makepad 输入框里正常合成~~ → **已验通**（§3） | ✅ 2026-10-16 |
| 气泡"贴文字" vs "通栏"：现在采用通栏（安全优先，§6.2）；要贴文字须按内容挑模板 + 窄窗保护 | 你决定要不要做（阶段 3） |
| Windows 上原生 TLS 是否撞证书吊销检查（兜底：换 rustls 客户端） | 阶段 2 实测 |
| Makepad 桌面打包工具是否存在/好用 | 阶段 4 核实 |
| iOS（需 Mac）、macOS/Linux（无机器） | 待条件 |
| 旧实现处置：Makepad 版可用后，Tauri 版 / egui 版删不删 | 你 |
| 视觉目标：是否要像素级贴近现在的网页版（工时差别很大） | 你 |

## 8. 约定（别违反）

- Decem 一侧统一叫 **char**，不写 bot / assistant（`MessageRole::Assistant` 是接口协议，保留）
- 网页版 `src/**` 只作对齐依据，**不为 Makepad 版改动它**
- Makepad 版是独立 crate（自带 `[workspace]`，根 `Cargo.toml` 里已 `exclude`），不拖累现有 CI
- `AGENTS.md` 只读，不改
