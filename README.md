# 🦊 Decem Chat

> 与赤狐 Decem 聊天的 AI Web 应用

# Decem 是谁？

是一只狐狸
![](https://valedecem.top/images/fox.png)

Decem Chat 是一个基于 **React 19 + TypeScript + Vite** 的前端应用，使用 **Netlify Edge Function** 代理请求到 **阿里云百炼 DashScope** 的聊天接口。应用支持聊天持久化、打字机动画、图片消息、主题切换和多模型选择。

## ✨ 核心功能

- **AI 聊天**：与 Decem 进行自然语言对话
- **安全代理**：前端请求 `/api/chat`，后端 Edge Function 使用服务器环境变量调用 AI API
- **逐字打字机动效**：收到完整回复后逐字呈现
- **图片消息**：支持向视觉模型发送图片输入
- **主题切换**：深色 / 浅色模式
- **聊天记录保存**：消息保存在浏览器 localStorage
- **设置面板**：切换模型、更新头像、清空聊天记录

## 📦 技术栈

- React 19 + TypeScript
- Vite
- Tailwind CSS
- Netlify Edge Functions
- 阿里云百炼 DashScope 兼容接口
- react-markdown + rehype-highlight
- localStorage + IndexedDB 图片存储

## 📁 目录结构

```
├── netlify/
│   └── edge-functions/
│       └── chat.ts          # Netlify Edge Function 代理 AI 请求
├── public/                  # 静态资源
├── src/
│   ├── components/          # UI 组件
│   ├── hooks/               # 自定义 Hook
│   ├── services/            # AI 和图片服务
│   ├── types/               # TypeScript 类型定义
│   └── main.tsx             # 应用入口
├── netlify.toml             # Netlify 构建和路由配置
├── package.json
└── tsconfig.json
```

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

### 3. 启动开发服务器

```bash
pnpm run dev
```

### 4. 构建生产版本

```bash
pnpm run build
```

### 5. 预览生产构建

```bash
pnpm run preview
```

## 🧩 Netlify 部署说明

项目使用 `netlify.toml` 配置：

- `build.command`: `pnpm run build`
- `publish`: `dist`
- `edge_functions`: 将 `/api/chat` 映射到 `netlify/edge-functions/chat.ts`

前端请求 `/api/chat`，Edge Function 负责将请求转发到 DashScope，保护 API Key 不暴露到浏览器。

## 🔐 安全说明

- **不要**将 API Key 写入前端环境变量 `VITE_...`
- 只能在服务端环境变量中配置 `API_KEY` 或 `NETLIFY_API_KEY`
- `.env` 文件应加入 `.gitignore`

## 🛰️ AI 接口实现

`netlify/edge-functions/chat.ts` 将收到的 POST 请求转发到：

- `https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions`

请求使用 `stream: false`，并返回完整结果给前端。

前端接口位于 `src/services/aiApi.ts`：

- 请求地址：`/api/chat`
- 发送模型 ID 和消息历史
- 解析返回的 `content`

## 🎛️ 运行说明

- 聊天消息和用户头像从浏览器本地保存
- 支持发送图片给视觉模型
- 回答内容在收到完整结果后呈现“打字机动画”效果

## 📌 其他说明

- 系统提示词位于：`src/services/SYSTEM_PROMPT.ts`
- 聊天 UI 组件在：`src/components/ChatInterface.tsx`
- 核心聊天逻辑在：`src/hooks/useChat.ts`

---

如果你需要快速部署到 Netlify，只需确保 `API_KEY` 配置好，并把仓库推送到 Netlify 即可。
