import 'dotenv/config';
import express from 'express';
import type {
  NextFunction,
  Request as ExpressRequest,
  Response as ExpressResponse,
} from 'express';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Readable } from 'node:stream';

const API_URL = 'https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions';

// Railway 会注入 PORT（动态端口）；本地默认 3000
const PORT = Number(process.env.PORT || 3000);
// 绑定所有网卡，确保容器外的代理网关（Railway / nginx / Cloudflare）能连上
const HOST = process.env.HOST || '0.0.0.0';

const app = express();

// 前面有 Cloudflare / Railway 网关的多层代理，信任第一层代理以正确识别客户端 IP
app.set('trust proxy', 1);

// 图片以 base64 内嵌在 JSON 请求体里，放宽体积限制
app.use(express.json({ limit: '20mb' }));

// 简单请求日志，便于在 Railway 日志里定位问题
app.use((req, _res, next) => {
  console.log(`${req.method} ${req.url}`);
  next();
});

// 健康检查：Railway / 负载均衡可直接用，返回 200 OK
app.get('/health', (_req, res) => {
  res.status(200).type('text/plain').send('OK');
});

// Express 4 不会自动捕获 async 路由里的异常，这里包一层交给全局错误中间件
const asyncHandler =
  (fn: (req: ExpressRequest, res: ExpressResponse, next: NextFunction) => Promise<void>) =>
  (req: ExpressRequest, res: ExpressResponse, next: NextFunction) => {
    fn(req, res, next).catch(next);
  };

// 聊天代理：保持契约（POST /api/chat，SSE 流式响应）
app.post(
  '/api/chat',
  asyncHandler(async (req, res) => {
    const apiKey =
      process.env.API_KEY || process.env.DASHSCOPE_API_KEY || process.env.NETLIFY_API_KEY;
    if (!apiKey) {
      res.status(500).json({ error: 'Server API key is not configured' });
      return;
    }

    const { model, messages, temperature } = (req.body ?? {}) as {
      model?: string;
      messages?: unknown[];
      temperature?: number;
    };

    if (!model || !Array.isArray(messages)) {
      res.status(400).json({ error: 'Missing model or messages' });
      return;
    }

    const upstreamBody: Record<string, unknown> = {
      model,
      messages,
      stream: true,
      max_tokens: 1500,
      enable_thinking: false,
    };
    if (typeof temperature === 'number') {
      upstreamBody.temperature = temperature;
    }

    let upstream: Response;
    try {
      upstream = await fetch(API_URL, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${apiKey}`,
        },
        body: JSON.stringify(upstreamBody),
      });
    } catch (err) {
      console.error('Failed to reach AI provider:', err);
      res.status(502).json({ error: 'Failed to reach AI provider' });
      return;
    }

    if (!upstream.ok || !upstream.body) {
      const text = await upstream.text().catch(() => '');
      let message = 'AI provider request failed';
      try {
        const data = JSON.parse(text);
        message = data?.error?.message || data?.message || message;
      } catch {
        // 忽略解析失败
      }
      res.status(upstream.status || 502).json({ error: message });
      return;
    }

    // 透传 SSE 流给浏览器，后端不再阻塞等待完整结果
    res.status(200);
    res.setHeader('Content-Type', 'text/event-stream; charset=utf-8');
    res.setHeader('Cache-Control', 'no-cache, no-transform');
    res.setHeader('Connection', 'keep-alive');
    // 告诉 nginx 等反向代理不要缓冲，保证流式实时性
    res.setHeader('X-Accel-Buffering', 'no');
    // 立即把响应头发出去，避免代理层等待超时
    res.flushHeaders?.();

    const nodeStream = Readable.fromWeb(
      upstream.body as unknown as import('node:stream/web').ReadableStream
    );
    nodeStream.on('error', (err) => {
      console.error('Upstream stream error:', err);
      res.destroy();
    });
    res.on('close', () => {
      upstream.body?.cancel().catch(() => {});
    });
    nodeStream.pipe(res);
  })
);

// 托管前端构建产物（同源部署）
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const distDir = path.resolve(__dirname, '../dist');
app.use(express.static(distDir));

// SPA 回退：非 /api 的 GET 请求一律返回 index.html
app.use((req, res, next) => {
  if (req.method !== 'GET' || req.path.startsWith('/api/')) {
    next();
    return;
  }
  res.sendFile(path.join(distDir, 'index.html'));
});

// 全局错误处理：任何未捕获异常都返回 500，避免进程崩溃或让代理层收到 502
app.use((err: unknown, _req: ExpressRequest, res: ExpressResponse, _next: NextFunction) => {
  console.error('Unhandled error:', err);
  if (res.headersSent) {
    res.destroy();
    return;
  }
  res.status(500).json({ error: 'Internal Server Error' });
});

// 显式绑定 0.0.0.0，确保代理网关能访问
app.listen(PORT, HOST, () => {
  console.log(`DecemChat server listening on http://${HOST}:${PORT}`);
  console.log(
    `API key configured: ${Boolean(
      process.env.API_KEY || process.env.DASHSCOPE_API_KEY || process.env.NETLIFY_API_KEY
    )}`
  );
});
