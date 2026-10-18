# Decem Chat 2.42.0 修复待办

> 依据：对 APK / DEX / `libdecem_shell_lib.so` / 内嵌前端 bundle 的静态分析。
> 当前工作区没有源码和构建环境，下面的路径按反编译结果推断（`src/`、`src-tauri/`），落到实际仓库时请核对。
>
> 优先级：**P0 = 发布前必须修**；**P1 = 本迭代**；**P2 = 后续**；**P3 = 工程化**。

## 0. 开工前确认

- [ ] 确认源码仓库位置，以及 `pnpm build`、`tauri android build` 可运行。
- [ ] 记录基线：APK `18,144,195` 字节；`.so` `16,520,784` 字节；DEX `1,469,264` 字节。
- [ ] 新建分支，例如 `fix/security-and-robustness`。
- [ ] 准备一个可用的测试上游（本地 mock 或测试环境），避免直接打生产接口。
- [ ] 确认发布包是 release 构建，而不是 debug。

---

## P0-1 阻断 Markdown 原始 HTML（安全，最高优先）

**证据**：前端 bundle 同时包含 `rehype-raw` 和 `rehype-highlight`，未发现 `rehype-sanitize`；`tauri.conf.json` 没有 `csp` 字段；自定义 Axum 服务器负责提供页面，Tauri 的 CSP 配置不一定覆盖它。

**文件**：`src/components/MessageBubble.tsx`（约 96–97 行的 Markdown 配置）、`src-tauri/src/server.rs`、`index.html`、`src-tauri/tauri.conf.json`

- [ ] **方案 A（推荐）**：如果产品不需要在模型输出里渲染原始 HTML，直接移除 `rehype-raw`，只保留 `remark-gfm` + `rehype-highlight`。
- [ ] **方案 B**：如果必须保留原始 HTML，加入 `rehype-sanitize`，并显式限制允许的标签/属性（禁止 `iframe`、`object`、`embed`、`style`、`script`、`form`、事件属性如 `onerror`/`onload`）。
- [ ] 插件顺序必须正确：`rehypeRaw` → `rehypeSanitize` → `rehypeHighlight`。顺序反了会把高亮用的 `className` 洗掉。
- [ ] 配置 CSP。注意页面由自定义服务器提供，优先在 `server.rs` 的 HTML 响应里加 `Content-Security-Policy` 头；同时在 `index.html` 放一个 `<meta http-equiv="Content-Security-Policy">` 作为兜底。
- [ ] CSP 基线：
  ```
  default-src 'self';
  script-src 'self';
  style-src 'self' 'unsafe-inline';
  img-src 'self' data: blob:;
  connect-src 'self';
  font-src 'self' data:;
  object-src 'none';
  base-uri 'none';
  frame-src 'none';
  frame-ancestors 'none';
  form-action 'none'
  ```
- [ ] 注意 `next-themes` 会注入一段内联脚本用于防闪烁；在严格 `script-src 'self'` 下会被拦。给它配置 `nonce`，或改用 CSP hash，尽量不要直接加 `'unsafe-inline'`。
- [ ] 移除 CSS 顶部的 Google Fonts `@import`，改用系统中文字体栈（`system-ui, "Noto Sans SC", sans-serif`）。这一步同时解决国内网络下字体请求超时的问题。

**验收**

- [ ] XSS 用例全部失效：`<script>alert(1)</script>`、`<img src=x onerror=...>`、`<iframe srcdoc="<script>...</script>">`、`<svg onload=...>`、`[x](javascript:alert(1))`。
- [ ] 正常 Markdown 不受影响：表格、列表、引用、代码块、行内代码、链接、代码高亮。
- [ ] DevTools Console 能看到 CSP 拦截记录；页面无内联脚本报错。

---

## P0-2 让请求真正可取消，并加超时

**证据**：代码创建了 `AbortController` 并调用 `.abort()`，但整个 bundle 中没有任何 `.signal`；`fetch` 只传了 `method`/`headers`/`body`。取消逻辑实际是死代码。

**文件**：`src/components/ChatInterface.tsx`（发送/重新生成回调）、`src/lib/api.ts`（若已抽出）

- [ ] 每次请求创建独立 `AbortController`，把 `signal: controller.signal` 传进 `fetch`。
- [ ] 发送新消息或点击“停止”时，先 `abort()` 上一个 controller。
- [ ] 加应用层超时，例如 120 秒；超时用 `controller.abort(new DOMException("timeout", "TimeoutError"))`，并在 `finally` 里 `clearTimeout`。
- [ ] 流式读取循环中检查 `signal.aborted`，及时退出。
- [ ] 区分用户取消和真实错误：`AbortError` 不弹“API 请求失败” toast。
- [ ] 如果产品需要“停止生成”，在 UI 上暴露停止按钮；如果不需要，删掉无用的 `AbortController`，避免误导。

**验收**

- [ ] 请求进行中再次发送/点击停止，DevTools Network 显示请求 `(canceled)`。
- [ ] 上游挂起时，120 秒后前端结束等待并给出超时提示。
- [ ] 用户主动取消不产生错误 toast。

---

## P0-3 图片压缩与请求体预算

**证据**：上传限制 5MB，但图片原样转 data URL；消息构造固定取最近 10 条，且其中所有图片都会重新发送。5 张 5MB 图片的 base64 约 33MB，会超过后端 32MiB 请求体上限。

**文件**：`src/components/ChatInput.tsx`（图片选择/预览）、`src/components/ChatInterface.tsx`（消息构造）、`src/lib/images.ts`（建议新建）

- [ ] 非 GIF 图片先用 canvas 缩放：最长边约 1568px，JPEG/WebP 质量 0.8–0.85；截图/文字类可优先 PNG。
- [ ] IndexedDB 里保留原图用于预览，发送时用压缩后的副本。
- [ ] 发送前计算 `JSON.stringify(body)` 的近似字节数，设置预算（建议 20–24MB，低于后端 32MiB）。
- [ ] 超预算时从最旧的消息开始，把图片替换为 `[图片]` 文本，直到达标。
- [ ] 把 5MB 限制的错误提示从 `alert()` 改为现有 toast 组件。
- [ ] 加一个“当前请求大小”的开发模式日志，便于回归。

**验收**

- [ ] 连续发送 6 张 5MB 图片后，请求仍成功，不发生 413。
- [ ] 普通照片发送后，视觉模型能正常识别，画质可接受。
- [ ] GIF 抽帧、预览、清空会话后的 IndexedDB 清理仍然正常。

---

## P0-4 传输安全

**文件**：`src-tauri/src/proxy.rs`、`AndroidManifest.xml`

- [ ] release 包关闭 `android:usesCleartextTraffic`，或改成仅 debug 生效的 network security config。
- [ ] `DECEMCHAT_API_URL` 覆盖仅在 debug 构建启用；release 强制 HTTPS。
- [ ] 本地代理保持只绑定 `127.0.0.1`；如需防止其他本机应用滥用，可在 Rust 启动时生成随机 token，通过 WebView 初始化脚本注入前端，请求时校验该 header。

**验收**

- [ ] release 包无法用 `http://` 上游。
- [ ] 正常 HTTPS 请求不受影响。

---

## P1 发布工程（性价比高，建议本迭代做完）

**文件**：`src-tauri/Cargo.toml`、前端构建配置、`src/index.css`

- [ ] 在 `Cargo.toml` 加：
  ```toml
  [profile.release]
  strip = "symbols"
  lto = true
  codegen-units = 1
  panic = "abort"
  ```
  保留 `.dynsym`（JNI 需要），去掉 `.symtab`/`.strtab`，预计 APK 减少约 5MB。
- [ ] 移除生产构建里的 `code-path` 注入插件（190 处，泄露 Windows 源码路径和行号）；只在 dev 开启。
- [ ] 移除或按环境关闭 `console.error`/`console.warn` 的生产输出。
- [ ] 用 `crypto.randomUUID()` 替代 `Date.now().toString()` 作为消息 ID。
- [ ] 评估启用 App Bundle / ABI split，减少安装体积。
- [ ] 把 NDK 从 r29-beta3 固定到 stable 版本；Java source/target 从 1.8 升到 17（确认 Tauri 模板兼容）。
- [ ] 检查 Tauri `EmbeddedAssets` 与自定义 `server::assets::DIST` 是否真的需要两份；能复用就复用。

**验收**

- [ ] release APK 体积明显下降，且安装、启动、聊天、图片、清空会话全部正常。
- [ ] 生产 bundle 中不再出现 `code-path` 和 `src\...tsx` 字样。

---

## P2 健壮性与产品一致性

- [ ] 流式响应里处理 `event: error` 和 `{"error": ...}`，不要把“空内容”统一报成“服务器未返回有效内容”。
- [ ] 统一错误模型：网络错误、超时、HTTP 状态码、上游业务错误分别提示。
- [ ] `contextWindow` 字段当前出现 10 次且全部是定义，没有任何读取；要么真正用于裁剪上下文，要么删掉，避免“1M 上下文”成为误导。
- [ ] 重新评估“固定只发最近 10 条消息”的策略：至少按 token/请求体预算裁剪，而不是固定条数。
- [ ] 清理内置 system prompt 中与现实不符的内容：“当前在QQ聊天”、被问模型名回答“DeepSleep-V5”、否认 AI。这是产品/合规决策，建议单独评审。
- [ ] 加 React error boundary，避免单个渲染异常白屏。
- [ ] 评估是否需要崩溃/错误上报；如果要加，先更新隐私说明。

---

## P3 工程化

- [ ] 抽出纯函数并补单测：消息构造、SSE 解析、图片预算、错误映射（Vitest）。
- [ ] Rust 侧补路由测试：`POST /api/chat`、32MiB body limit、静态资源 fallback、响应 header（`tower::ServiceExt` + mock 上游）。
- [ ] CI：`tsc --noEmit`、ESLint、`cargo fmt --check`、`cargo clippy`、`cargo test`、`pnpm build`、Android 打包。
- [ ] 依赖审计：`pnpm audit`、`cargo audit`。
- [ ] 发布检查清单：版本号、签名、CSP、debug 开关、体积、手动回归矩阵。

---

## 回归清单（每个 P0 完成后都跑一遍）

| 场景 | 期望 |
| --- | --- |
| 普通文本对话 | 流式输出正常，Markdown 正常 |
| 上游非 SSE JSON | 能正确取 `content` / `choices[0].message.content` |
| 上游返回 4xx/5xx | 显示可读错误，不白屏 |
| 流式过程中取消 | 请求 canceled，无错误 toast |
| 上游挂起 | 超时后恢复可操作 |
| JPG/PNG/GIF 上传 | 预览、识别、抽帧正常 |
| 5MB 边界与超限 | 5MB 可发；超限提示友好 |
| 多图长会话 | 请求体不超 32MiB |
| XSS 用例 | 全部失效或转义 |
| 清空会话 | localStorage、IndexedDB 图片都清理 |
| 杀进程重开 | 最近 50 条消息恢复，图片能回填 |
| 深色/浅色切换 | 无闪烁、无 CSP 报错 |

---

## 代码位置速查

| 位置 | 内容 |
| --- | --- |
| `src/components/MessageBubble.tsx:96-97` | Markdown 插件与组件映射（`rehypePlugins`） |
| `src/components/ChatInterface.tsx` | 发送/重新生成、`AbortController`、消息构造 `slice(-10)` |
| `src/components/ChatInput.tsx` | 图片选择、5MB 限制、GIF 抽帧入口 |
| `src/components/SettingsPanel.tsx` | 模型选择、头像、深色模式、清空会话 |
| `src-tauri/src/proxy.rs` | 上游请求、`bytes_stream` 流式转发、响应 header |
| `src-tauri/src/server.rs` | Axum router、静态资源、body limit（32MiB）、CSP 应加在这里 |
| `src-tauri/tauri.conf.json` | CSP、窗口、安全配置 |
| `src/index.css` | Google Fonts `@import`、Tailwind/highlight 样式 |

## 建议开工顺序

1. **P0-1 + P0-2**：安全和取消逻辑，改动小、收益最大。
2. **P0-3**：图片压缩与请求预算，避免真实用户踩 413。
3. **P1 的 strip + code-path**：快速减小体积、清理生产痕迹。
4. **P0-4 + P2**：传输安全和错误模型。
5. **P3**：补测试和 CI，防止回归。
