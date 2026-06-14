# 🦊 Decem Chat

> 与一只住在奇幻森林里的赤狐聊天

Decem Chat 是一个基于 React 的 AI 对话网页应用，通过与阿里云百炼平台的多款大语言模型对接，让你可以和 Decem——一只拟人化的赤狐 OC 角色——进行有趣的对话。

![Preview](public/images/decem.png)

## ✨ 功能特点

- **流式对话** — AI 回复逐字显示，体验流畅自然
- **图片发送** — 可以发送图片给支持视觉理解的模型
- **深色/浅色模式** — 一键切换主题
- **聊天记录持久化** — 自动保存到 localStorage，刷新不丢失
- **自定义头像** — 上传自己的头像
- **多模型切换** — 在多款大模型之间自由切换
- **设置面板** — 玻璃态效果的侧边栏设置

## 🤖 支持的模型

| 模型 | 描述 | 图片 | 视频 |
|------|------|:----:|:----:|
| ⚡ **Qwen3.6-Flash** | 像闪电一样快的小狐狸！ | 👁️ | 🎬 |
| 🧠 **Qwen3.6-Plus** | 更聪明的大狐狸！推理和复杂任务更强 | 👁️ | 🎬 |
| 🏆 **Qwen3.7-Max** | 森林里的智者！综合能力最强 | — | — |
| 🌐 **Qwen3.5-Omni-Plus** | 全能的旅行狐狸！多语言和跨领域知识都擅长 | 👁️ | 🎬 |
| 🐳 **DeepSeek-V4-Pro** | 人坏，小鲸鱼好 | — | — |
| 🐳 **DeepSeek-V4-Flash** | 小鲸鱼最好了 | — | — |
| 🌕 **Kimi-K2.6** | 寻求将能源转化为智能的最优解 | 👁️ | 🎬 |
| 🐼 **GLM-5.1** | 整理竹子的熊猫 | — | — |

## 🛠️ 技术栈

- **前端框架**: React 19 + TypeScript
- **构建工具**: Vite
- **样式**: Tailwind CSS + shadcn/ui
- **AI API**: 阿里云百炼 (DashScope) OpenAI 兼容接口
- **流式处理**: Fetch API + ReadableStream
- **数据持久化**: localStorage

## 📁 项目结构

```
├── src/
│   ├── components/         # React 组件
│   │   ├── ChatInterface.tsx   # 主聊天界面
│   │   ├── ChatInput.tsx       # 输入框组件
│   │   ├── MessageBubble.tsx   # 消息气泡组件
│   │   └── SettingsPanel.tsx   # 设置面板
│   ├── hooks/              # 自定义 Hooks
│   │   ├── useChat.ts          # 聊天逻辑
│   │   └── useSettings.ts    # 设置管理
│   ├── services/           # API 服务
│   │   └── aiApi.ts            # 阿里云百炼 API
│   ├── types/              # TypeScript 类型
│   │   └── index.ts
│   └── index.css           # 全局样式
├── public/
│   └── images/             # 头像图片
├── .gitignore
└── vite.config.ts
```

## 🚀 本地开发

### 1. 克隆项目

```bash
git clone https://github.com/defaultuser233/DecemChat.git
cd DecemChat
```

### 2. 安装依赖

```bash
pnpm install
```

### 3. 配置 API Key

设置环境变量

阿里云百炼 API Key 请在 https://bailian.console.aliyun.com/ 获取

```bash
export VITE_API_KEY=sk-XXXXXXXX
```

### 4. 准备头像图片

将头像图片放到 `public/images/` 目录下：

- `decem.png` — Decem 的头像
- `default.png` — 用户的默认头像

### 5. 启动开发服务器

```bash
pnpm run dev
```

### 6. 构建生产版本

```bash
pnpm run build
```

### 7. 预览生产分支

```bash
pnpm run preview
```

## ⚙️ 环境变量

| 变量名 | 说明 | 必填 |
|--------|------|:----:|
| `VITE_API_KEY` | 阿里云百炼 API Key | ✅ |

获取方式：登录 [阿里云百炼控制台](https://bailian.console.aliyun.com/)

## 📝 提示词

Decem 的角色设定和说话风格通过系统提示词（System Prompt）注入，位于 `src/services/SYSTEM_PROMPT.ts` 中。

核心设定：
- 身份：拟人化的赤狐，不是 AI 助手
- 语气：软萌简短，爱用"嗷呜~"
- 格式：开头带 🦊，结尾带"嗷呜~"
- Emoji：主要使用 🦊🧣🍃❤️✨🥺
- 心理活动：用 `()` 包含，每轮至少一处

## 🔒 安全说明

- API Key 保存在 `.env` 文件中
- `.env` 已加入 `.gitignore`，**不要**提交到 Git
- 聊天记录保存在用户浏览器本地，不会上传到服务器

---

🦊 和狐狐聊天愉快！嗷呜~
