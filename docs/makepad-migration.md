# Makepad 迁移：状态、计划与作业手册

> 更新：2026-10-16　关联文档：`docs/makepad-probe.md`（探针实测记录与候选筛选）
> 这份文档的目的：**换一个会话、隔几周回来，也能立刻接着干**。

## 0. 一句话状态

方向已定：**用 Makepad（纯 Rust 自绘）重做 Decem Chat**，分四阶段、每阶段有验收门槛。
当前进度：**阶段 1 的代码已写完并编译通过**（`decem-makepad/`；首次编译修了一处借用错误），
**尚未运行与截图**——下一步就是跑起来看效果，以及等你在 Windows 上验中文输入法。
现有实现（网页版 / Tauri 版 / egui 版）**全部保留、一行未动**，继续是主力。

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

## 3. 阶段 1 现状

代码：`decem-makepad/src/main.rs`（约 300 行）

- `PortalList` 消息列表（左右两种气泡模板，`Char` 左/深色，`User` 右/橙 `#xE95620`）
- `glass.TextInput` + `glass.GlassButtonProminent`，回车发送、Esc 中断
- **模拟上游**：工作线程按 3 字符/28ms 吐块 → `SignalToUI` → UI 轮询 → Markdown 流式动画逐字显形
- `DECEM_STRESS=1` 启动时种 300 条消息，用于压测滚动

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

> 读文本 `input.text()`、写文本 `input.set_text(cx, ...)`（`aichat:679-716`）。

## 6. 必读的三个坑（完整版见 `docs/makepad-probe.md` §5）

1. **不装暗色风格** → 行内代码在高亮块上几乎看不见（浅色主题压自绘深底）
2. **Markdown 表格必须 `width: Fill`**，用 `Fit` 会被压成零宽（表现为"气泡里留空白"）
3. **气泡底色别与行内代码底色用同一个主题变量**（`theme.color_bg_highlight` 会糊在一起）

## 7. 未决 / 只有人能验的

| 项 | 谁 |
| --- | --- |
| **中文输入法（IME）能否在 Makepad 输入框里正常合成** | 你（阶段 1 的门槛） |
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
