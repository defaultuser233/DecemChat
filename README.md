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
├── public/                  # 静态资源
├── src/
│   ├── components/          # UI 组件
│   ├── hooks/               # 自定义 Hook
│   ├── services/            # AI 和图片服务
│   ├── types/               # TypeScript 类型
│   └── main.tsx             # 应用入口
├── netlify.toml             # Netlify 配置
├── package.json
└── tsconfig.json
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