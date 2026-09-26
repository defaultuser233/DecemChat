# 🦊 Decem Chat

> 与赤狐 Decem 聊天的 AI Web 应用

# Decem 是谁？

是一只狐狸
![](https://valedecem.top/images/fox.png)

Decem Chat 是一个基于 **React 19 + TypeScript + Vite** 的前端聊天应用。前端通过 **Netlify Edge Function** 代理请求到 **阿里云百炼 DashScope**，确保 API Key 安全存放在服务端。

## ✨ 主要介绍

- **AI 聊天**：与 Decem 进行自然语言对话
- **安全代理**：前端请求 `/api/chat`，Edge Function 负责转发并保护 API Key
- **打字机动画**：收到完整回复后逐字显示
- **图片消息**：支持向视觉模型发送图片输入
- **主题切换**：深色 / 浅色模式
- **消息持久化**：聊天记录保存在浏览器 localStorage
- **设置面板**：可以切换模型、更新头像、清空聊天记录

## 📦 技术栈

- React 19 + TypeScript
- Vite
- Tailwind CSS
- Netlify Edge Functions
- 阿里云百炼 DashScope 兼容接口
- react-markdown + rehype-highlight + remark-gfm
- localStorage + IndexedDB 图片存储

## 📁 项目结构

```text
├── netlify/
│   └── edge-functions/
│       └── chat.ts          # Netlify Edge Function 代理 AI 请求
├── public/                  # 静态资源（Web 与桌面版共用）
├── src/
│   ├── components/          # UI 组件
│   ├── hooks/               # 自定义 Hook
│   ├── services/            # AI 和图片服务
│   ├── types/               # TypeScript 类型
│   └── main.tsx             # 应用入口
├── crates/                  # 🦀 Rust 桌面版（Cargo 工作区）
│   ├── decem-core/          # 平台无关核心：模型表、提示词、SSE、DashScope 客户端
│   └── decem-app/           # egui 桌面应用
├── .github/workflows/
│   └── rust.yml             # Rust 全平台构建矩阵
├── Cargo.toml               # Rust 工作区清单
├── netlify.toml             # Netlify 配置
├── package.json
└── tsconfig.json
```

## 🦀 Rust 桌面版（全平台原生应用）

除了 Web 版，仓库里还有一份 **纯 Rust 原生桌面版**：`crates/` 下的 Cargo 工作区，
编译出 Windows / macOS / Linux 的原生可执行文件，不需要浏览器、也不需要 Node 运行时。

- **跨平台产物**：Windows（x64 / arm64）、macOS（Intel / Apple Silicon）、Linux（x64 / arm64）
- **渲染**：eframe + egui（GPU 加速），中文自动从系统字体加载，emoji 用内置彩色字体
- **功能对齐 Web 版**：打字机动画、Markdown + 代码高亮、图片消息、GIF 抽帧拼接、
  深色 / 浅色、模型切换与视觉能力标注、头像上传、重新生成、清空记录（含二次确认）
- **本地持久化**：设置 / 聊天记录 / 图片存进系统配置目录，替代 localStorage + IndexedDB
- **直连模型**：桌面版没有服务端，API Key 在设置面板填写或读环境变量

### 构建与运行

```bash
cargo run -p decem-app                       # 直接运行
cargo build --release -p decem-app           # 产出本平台可执行文件
cargo test --workspace                       # 单元测试
cargo run -p decem-app -- --selftest         # 无界面自检（存储往返 + 请求编排）
cargo run -p decem-app -- --selftest --live  # 再真发一次请求，验证端到端链路
```

六个平台目标的构建矩阵见 `.github/workflows/rust.yml`（Windows / macOS / Linux × x64 / arm64）。

### 数据与配置位置

| 内容 | 位置 |
| --- | --- |
| 设置与 API Key | `<配置目录>/config.json` |
| 聊天记录（最近 50 条） | `<配置目录>/messages.json` |
| 图片（data URL） | `<配置目录>/images/<消息ID>.b64` |
| 运行日志 | `<配置目录>/decemchat.log` |

配置目录由系统决定：Windows 为 `%APPDATA%\valedecem\DecemChat\config`，
macOS 为 `~/Library/Application Support/top.valedecem.DecemChat`，
Linux 为 `~/.config/decemchat`。



### 图标与字体

图标不是手绘的：`crates/decem-app/src/ui/lucide_data.rs` 由
`node scripts/gen-lucide-icons.mjs` 从 `node_modules` 里的 **lucide-react**（网页版用的同一套）
提取几何数据，再由 `ui/svg_path.rs` 解析 SVG 路径画出来，所以与网页版是同一份矢量。

字体方面：

- 中文按平台探测（Windows 微软雅黑 / macOS 苹方 / Linux Noto Sans CJK、文泉驿），
  可用 `DECEMCHAT_FONT` 覆盖 —— 中文字体体积太大，不适合内置
- emoji **内置**了两份 Noto（`crates/decem-app/assets/`，SIL OFL 1.1，许可全文见同目录 `OFL.txt`）：
  `NotoEmoji.ttf`（单色，走文本排版）与 `NotoColorEmoji.ttf`（彩色，走图片），
  两者字形同源，混排也不违和；可用 `DECEMCHAT_EMOJI_FONT` 换掉单色那份

> **字体链的顺序不能随便挪**：中文字体必须插在拉丁字体**后面一位**。
> egui 自带的 `NotoEmoji-Regular`、`emoji-icon-font` 也排在拉丁字体后面，
> 而它们里面带着 `？`、`！`、`～` 这些全角标点 —— 中文排到它们之后，
> 标点就被它们抢走，画出来又粗又小（网页版是交给系统中文字体的）。
> `fonts.rs` 的 `Fallback::AfterLatin` 管这件事，`fonts.rs` 里有测试盯着。

**彩色 emoji 是怎么画出来的**（egui 的字体管线只取 skrifa 的 `OutlineGlyphCollection`，
`epaint-0.36.2/src/text/font.rs:200`，读不到 COLR/CBDT 位图，所以不能走字体）：

> 本文里 `epaint-0.36.2/…`、`egui_commonmark-0.25.0/…` 这类路径都相对于 **cargo 依赖源码根**。
> 那个前缀每台机器可能不同（`CARGO_HOME` 可配），要拿到绝对路径就跑：
> `cargo metadata --format-version 1 | tr ',' '\n' | grep manifest_path`。

1. 渲染 Markdown 前，把正文里成对的 emoji 换成一个内联 span `$emoji:1f98a$`
   （`src/emoji.rs` 的 `to_render_markdown`，代码块与行内代码原样保留）；
2. `egui_commonmark` 把它当行内公式，回调 `render_math`
   （`egui_commonmark-0.25.0/src/parsers/pulldown.rs:576` 的 `math_fn` 分支）；
3. 回调里用 skrifa 从 `NotoColorEmoji.ttf` 的 CBDT 表取出该字的 PNG 位图，
   按当前 `pixels_per_point` 缩放成纹理画出来，同一尺寸只光栅化一次。

尺寸按网页版量出来的比例定：位图框画 1.4 em、墨迹落在 0.88 em 上下。

气泡宽度也不再靠字号估算：`content_width` 会**屏幕外真实渲染一遍**，量出「不换行时最宽一行」
（网页版 `inline-block` 的 `max-content` 就是这个），再按上限截断 —— 所以标题、列表、表格、
代码块、emoji 都量得准，短消息右侧不会再空出一截。结果按（内容哈希, DPI, 排版环境指纹）缓存，
正常帧不会多渲染一遍（指纹是必要的：`set_fonts` 要下一趟才生效，第一趟量出来的宽度偏小）。

> **一处仍与网页版不同**：**代码块与行内代码里的 emoji 还是单色**。
> 代码块用的是 `egui::TextEdit`（`egui_commonmark_backend-0.25.0/src/elements.rs:105`），
> `Galley` 里塞不进图片；要在那里也画彩色，只能连 egui_commonmark 一起改成手工铺字，
> 代价是丢掉代码块的文本选中，不划算。其余位置（正文、标题、列表、引用、表格、
> 打字机动画、欢迎语、设置抽屉里的模型说明）都是彩色。

> **另两处点名要的偏离**（都记在 `theme.rs`，别按网页版「改回去」）：
>
> - 输入区那两个按钮用橙色 `#E95620`（图标常驻橙色，发送键能发时橙底白图标、
>   空闲时暗底保留「还不能发」的提示）。网页版这里是珊瑚色 `#F38181` 与灰底。
> - **两侧气泡都四角统一**（`18px`）。网页版两侧靠头像那一角都会收窄成 4px 的「尾巴」
>   （用户侧右下、Decem 侧左下），用户说那个尖角不好看，两边都改圆了。

### 接口地址与 API Key

**默认走本项目自己的服务端代理**（`http://api.valedecem.top:3000/api/chat`，网页版用的同一个），
API Key 留在服务器的 `.env` 里，**客户端不需要配置任何密钥**。

接口地址的解析顺序：`config.json` 的 `apiUrl` → 环境变量 `DECEMCHAT_API_URL` → 上面的默认值。
想直连百炼：把 `apiUrl` 显式设成空字符串，再用下面任一方式提供 Key：

- 首次启动时弹出的填写框（只在「直连且没 Key」时出现，填过之后设置面板与网页版完全一致）
- 环境变量 `API_KEY` / `DASHSCOPE_API_KEY` / `NETLIFY_API_KEY`（按此顺序）
- `config.json` 的 `apiKey` 字段

自检可以直接验证整条链路：

```bash
cargo run -p decem-app -- --selftest --live   # 默认走服务端代理，无需 Key
```

## 🧩 Netlify 部署说明

项目使用 `netlify.toml` 配置：

- `build.command = "pnpm run build"`
- `publish = "dist"`
- `edge_functions` 将 `/api/chat` 映射到 `netlify/edge-functions/chat.ts`

部署后，前端对 `/api/chat` 的请求会由 Edge Function 转发到 DashScope。

## 🔐 安全说明

- **不要**将 API Key 写入前端环境变量 `VITE_...`
- 只在服务端环境变量中配置 `API_KEY` 或 `NETLIFY_API_KEY`
- `.env` 文件应加入 `.gitignore`

## 🛰️ AI 接口实现

`netlify/edge-functions/chat.ts` 转发 POST 请求到：

- `https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions`

请求采用 `stream: false`，并返回完整结果给前端。

前端请求实现于 `src/services/aiApi.ts`：

- 请求地址：`/api/chat`
- 发送模型 ID 和消息历史
- 解析并返回 `content`

## 🎛️ 运行说明

- 聊天消息和用户头像在浏览器本地保存
- 支持将图片发送给视觉模型
- 回复先获取完整文本，再逐字显示为打字机动画

## 📌 关键文件

- 系统提示词：`src/services/SYSTEM_PROMPT.ts`
- 聊天逻辑：`src/hooks/useChat.ts`
- UI 入口：`src/components/ChatInterface.tsx`
- 后端代理：`netlify/edge-functions/chat.ts`

---

请先确认 `API_KEY` 已配置，再使用 `pnpm exec netlify dev` 进行本地调试。

## 🚀 本地开发

### 1. 安装依赖

```bash
pnpm install
```

### 2. 配置环境变量

在项目根目录创建 `.env` 或使用 Netlify 环境变量。必须设置：

```bash
API_KEY=sk-xxxxxxxxxxxxxxxxxxxx
```

> Edge Function 会读取以下环境变量之一：`API_KEY`、`DASHSCOPE_API_KEY`、`NETLIFY_API_KEY`

### 3. 登录 Netlify，并将当前项目与 Netlify 站点绑定

```bash
pnpm exec netlify login
```

```bash
pnpm exec netlify init
```

### 4. 启动开发服务器，本地测试 Netlify Edge Function

```bash
pnpm exec netlify dev
```

> pnpm run dev 将不会使用Netlify Edge Function

### 5. 构建生产版本

```bash
pnpm run build
```

### 6. 预览生产构建

```bash
pnpm run preview
```

### 7. 部署到 Netlify

部署生产分支：
```bash
pnpm exec netlify deploy --prod
```
或先测试预发布：
```bash
pnpm exec netlify deploy --dir=dist
```

---

### 🧣祝你和狐狐玩得开心！

![](https://valedecem.top/images/Domain.jpg)