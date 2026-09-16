# DecemChat 生产镜像（Railway / 任意 Docker 环境）
FROM node:22-slim

WORKDIR /app

# 启用 pnpm（会读取 package.json 的 packageManager 字段）
RUN corepack enable

# 先复制依赖清单，利用 Docker 层缓存
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

# 复制源码并构建前端（tsc -b && vite build）
COPY . .
RUN pnpm build

ENV NODE_ENV=production
EXPOSE 3000

# 启动后端（同时托管 dist 静态资源 + /api/chat）
CMD ["pnpm", "start"]
