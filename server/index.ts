import 'dotenv/config';
import express from 'express';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Readable } from 'node:stream';

const API_URL = 'https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions';
const PORT = Number(process.env.PORT || 3000);

const app = express();

// 图片以 base64 内嵌在 JSON 请求体里，放宽体积限制
app.use(express.json({ limit: '20mb' }));

// 聊天代理：保持与原 Netlify Edge Function 相同的契约（POST /api/chat，SSE 流式响应）
app.post('/api/chat', async (req, res) => {
  const apiKey = process.env.API_KEY || process.env.DASHSCOPE_API_KEY || process.env.NETLIFY_API_KEY;
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
  } catch {
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
  res.setHeader('Cache-Control', 'no-cache');
  res.setHeader('Connection', 'keep-alive');
  // 告诉 nginx 等反向代理不要缓冲，保证流式实时性
  res.setHeader('X-Accel-Buffering', 'no');

  const nodeStream = Readable.fromWeb(
    upstream.body as unknown as import('node:stream/web').ReadableStream
  );
  nodeStream.on('error', () => res.destroy());
  res.on('close', () => {
    upstream.body?.cancel().catch(() => {});
  });
  nodeStream.pipe(res);
});

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

app.listen(PORT, () => {
  console.log(`DecemChat server listening on http://localhost:${PORT}`);
});
