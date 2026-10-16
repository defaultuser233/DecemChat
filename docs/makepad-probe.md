# Makepad 探针记录：纯 Rust 自绘 + 全平台这条路的实证

> 日期：2026-10-16
> 探针源码：`D:\Documents\MakepadProbe\decem-probe`（path 依赖版）、`decem-probe-git`（git 依赖版）
> Makepad 仓库：`D:\Documents\MakepadProbe\makepad`（dev 分支，commit `71c9ddcb`）
> 探针 APK：`D:\Documents\MakepadProbe\decem-probe-android.apk`

**结论先行**：在「纯 Rust、自绘（不用 WebView）、覆盖 Windows/macOS/Linux/Android/iOS」这组条件下，
**生态里只剩 Makepad 一个候选**。而实测下来，我们在 egui 上吃过的苦（彩色 emoji、中文、全角标点、
气泡宽度）**在 Makepad 里四个都不是问题**，代价是：生态年轻（crates.io 落后约 16 个月）、
工具链自成一体系且笨重、以及**聊天 App 除地基之外的一切都要从零写**。

---

## 1. 为什么要评估

Decem 现有的两条实现：

| 实现 | 状态 | 问题 |
| --- | --- | --- |
| 网页版（React + Vite） | 线上运行 | 依赖浏览器，无法做原生桌面/移动 |
| 桌面版（Rust + egui） | 六平台 CI 全绿、可用 | 纯自绘，但为「复刻网页版设计」付了很大代价（见 §4） |
| Tauri 壳（React UI + 本地服务） | 已出 Windows 安装包与 18MB Android release APK | 依赖系统 WebView，不是「纯自绘」 |

于是问题变成：**如果坚持「纯 Rust + 自绘」这条技术路线，今天还有谁能覆盖五个平台？**

## 2. 候选筛选（2026-09 核实，均附来源）

| 框架 | 五平台 | 纯自绘 | 判定与卡点 |
| --- | --- | --- | --- |
| **Makepad** | ✅ | ✅ | **唯一全中**。`platform/Cargo.toml` 有 `apple-ios`/`android`/`ohos`/tvos 的 target 段；MIT/Apache；卡点是 crates.io 落后与生态年轻 |
| Slint | ✅ | ✅ | 五平台与自绘都成立，但 UI 用 `.slint` DSL（不是纯 Rust 代码），且免版税档**要求署名**（AboutSlint/徽章），不含嵌入式 |
| egui / eframe | ⚠️ 4.5 | ✅ | 桌面 ✅、Android 官方声明支持（需 `android-game-activity` + 手写入口）、**iOS 是未文档化的降级路径** |
| Dioxus | ✅ | ❌ | 生产路径是 **WebView**；自绘（Blitz/Vello）官方自评 experimental / beta / "bleeding edge" |
| Fission | ✅ | ✅ | 功能对得上（九目标、Vello/WGPU），但 2026-04 才创建、总下载 1207、27 star、**单人 1571 次提交**、CI 无 Windows/macOS 桌面作业 |
| Iced | ❌ | ✅ | 官方自称 "experimental software"，平台列表无移动端 |
| gpui | ❌ | ✅ | **不支持 Windows**，pre-1.0，Zed 自用 |
| Xilem | ❌ | ✅ | 官方自称 alpha，无移动端 |
| Freya / Floem | ⚠️ | ✅ | 0.x，Freya 有 `freya-android` 依赖但无官方支持声明 |

## 3. 实测记录

### 3.1 环境

- Windows 11 + D3D11（Makepad 在 Windows 走 d3d11）；rustc/cargo 1.98.1
- 截图取证：`PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT=2)` **能抓到 GPU 渲染的窗口内容**（这一点很关键，否则没法留证）
- Android：本机已有 Android Studio（JBR 21）+ 自建 SDK `D:\Android\Sdk`

### 3.2 Windows

| 项目 | 结果 |
| --- | --- |
| 官方示例 `makepad-example-hello-world` | ✅ 1 分 19 秒编译完成、运行正常 |
| 本探针（path 依赖） | ✅ 1 分 37 秒编译完成、运行正常 |
| 本探针（**git 依赖**，真项目形式） | ✅ 1 分 53 秒（cargo 拉 dev 分支、锁定 132 个包） |
| 渲染质量 | GPU 渲染 + **次像素抗锯齿**（大标题可见 RGB 边缘） |

### 3.3 Android

工具链是 `cargo-makepad`（从仓库现编，18.8 秒）：

```bash
# 1) 编工具
cargo install --path=./tools/cargo_makepad          # 在 makepad 仓库根目录

# 2) 装工具链（注意：它会往 --sdk-path 指的目录里**加自己的一套**）
cargo makepad android --abi=aarch64 --sdk-path='D:/Android/Sdk' \
  --package-name=top.valedecem.probe --app-label='Decem Probe' install-toolchain

# 3) 出包（必须带 -p）
cargo makepad android --abi=aarch64 --sdk-path='D:/Android/Sdk' \
  --package-name=top.valedecem.probe --app-label='Decem Probe' \
  build -p decem-probe --release
```

| 项目 | 结果 |
| --- | --- |
| 它下载的组件 | `platform-33-ext4`、`build-tools_r33.0.1`、`platform-tools_r33.0.3`、**NDK r28b（精简版）**、**JDK 17** —— 合计约 1.2 GB，**不复用系统的** |
| 产物 | `decemprobe.apk`，**40 MB**，74 秒编完 |
| 签名 | V3 Signer `C=US, O=Android, CN=Android Debug` → **可直接侧载** |
| 内容 | `lib/arm64-v8a/libmakepad.so`（23.4 MB）+ `assets/makepad/.../resources/*.ttf` |
| 打包的字体 | **合计 49.5 MB**：`LXGWWenKaiRegular/Bold`（19+18.5 MB，中文）、**`NotoColorEmoji`（10.6 MB）**、`IBMPlexSans×4`、`LiberationMono`、`fa-solid-900` |

> **要点**：中文与彩色 emoji 是**随包内置的字体**，不依赖设备字体、也没有运行时下载——
> 因此 Android 上的表现应当与 Windows 一致（待手机实测确认）。

### 3.4 依赖形式

| 形式 | 结果 |
| --- | --- |
| path（本地克隆） | ✅ 探针用的就是它 |
| **git（真项目形式）** | ✅ `makepad-widgets = { git = "...", branch = "dev", default-features = false, features = ["rich_text"] }` |
| crates.io | ✗ 线上是 1.0.0（2025-05），仓库已在 2.0.0 线；**富文本/Markdown 家族未发布**（`makepad-widgets-rich-text` 404）→ 真项目必须走 git |

⚠️ git 依赖下 cargo 会报 `skipping duplicate package bitflags/cfg-if ... in favor of ...`（仓库里 vendored 了
这些包的副本）——能用，但属于生态未成熟的信号。

## 4. 六个关键问题（与我们 egui 那版对照）

| 问题 | egui 版 | Makepad | 结论 |
| --- | --- | --- | --- |
| 彩色 emoji | 自写 CBDT 位图光栅化（epaint 只取轮廓字形） | **内置 `NotoColorEmoji.ttf`**，原生彩色 | ✅✅ |
| 中文 | 按平台探测系统字体路径 | **内置霞鹜文楷**（随包） | ✅ |
| 全角标点（？！～） | 被内置字体抢走 → 又粗又小，靠字体链顺序修 | 无此问题，与正文同字重 | ✅✅ |
| 气泡宽度 | 离屏真渲染量「最宽一行」（无 `max-content`） | **`width: Fit` 就是 `max-content`** | ✅✅ |
| Markdown | 借 `egui_commonmark` | 内置 `Markdown` 组件（标题/列表/表格/代码块/行内代码） | ✅ |
| 圆角气泡 | 手绘 | `draw_bg.border_radius: uniform(18.)` | ✅ |

额外确认：主题系统自带多套风格（含 `BlackOrange` = 近黑 + 橙）；`Markdown`/`TextFlow` 会自动换行，
**纯 `Label` 不会**。

## 5. 踩过的坑（复现时按此顺序）

1. **默认主题是浅色**（`Omarchy`）。不装风格就会出现「浅色高亮块压在自绘深色背景上、行内代码看不见」。
   装法：`desktop_style::install(vm, StyleSheet::load(DesktopStyle::BlackOrange))`（在 `script_mod` 里，定义 UI 之前）。
2. **Markdown 的表格必须 `width: Fill`**：表格内部走 Fill 布局，用 `Fit` 会被压成零宽（表现是「气泡里留空白，表格不见了」）。
3. **气泡底色别用 `theme.color_bg_highlight`**：行内代码的底色也是这个变量，会糊在一起。
4. **纯 `Label` 不换行**（长文本会被裁）；要换行用 `Markdown` 或 `TextFlow`。
5. `cargo makepad android build` **必须显式 `-p <crate>`**，否则报 `Not enough arguments to determine crate`。
6. **工具链安装会被系统 curl 的证书吊销检查打断**（`CRYPT_E_NO_REVOCATION_CHECK`，国内到 CRL/OCSP 不通）。
   它最后一步是下 `bundletool-all-1.18.1.jar`；而 **bundletool 只用于 `.aab`**，只出 APK 用不到。
   绕过：手工放到 `<sdk>/bundletool/bundletool.jar`，然后单独跑 `... expand-sdk`（zips 已下好）。
7. `--sdk-path` 指到**已有 SDK** 时，它会往里面塞自己那套组件（见 §3.3），不会复用你已有的。

## 6. 还没验证的

| 项 | 说明 |
| --- | --- |
| **iOS** | 官方有正路（`cargo makepad apple ios install-toolchain` + `run-sim`/`run-device`），但**必须有 Mac + Xcode** |
| macOS / Linux | 未测（同一份代码、官方支持；手上没有相应机器） |
| **Android 真机渲染** | APK 已产出且签名正确，但「装到手机上、中文/emoji/表格是否与 Windows 一致」未验 |
| 真实功能 | 探针是**纯 UI**：滚动列表、输入框、网络、图片、流式打字机都还没做 |

## 7. 如果真要用它重做 Decem，还差什么

- 消息列表：`PortalList`（Makepad 有滚动列表组件，`examples/portallist_hit` 可参考）
- 输入区：`TextInput`（`examples/text_input`）+ 发送按钮 + 回车发送
- 网络：SSE 流式请求（Makepad 有 `makepad-network`；我们的 `decem-core` 也可复用）
- 图片消息、头像、主题对齐（`BlackOrange` 与 Decem 的橙色系接近）、图标系统（它内置 FontAwesome 与 SVG 支持）
- **以及所有我们在 egui 版里已经解决过的产品细节**（气泡尾巴、悬停态、抽屉、Toast……）

**一句话**：地基已经验证成立；但从「地基成立」到「能替代现在这个 App」，工作量约等于把 egui 那版
重做一遍，只是不必再为字体/emoji/宽度这些底层问题付费。
