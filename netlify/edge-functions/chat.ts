const API_URL = 'https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions';

// 统一返回 JSON 响应，避免重复的 Content-Type 头与序列化代码
const jsonResponse = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });

export default async (request: Request) => {
  if (request.method !== 'POST') {
    return jsonResponse({ error: 'Method Not Allowed' }, 405);
  }

  // 兼容 Netlify Edge Functions 的 Deno 运行时与本地 Node 环境
  const getEnv = (name: string): string | undefined => {
    const g = globalThis as unknown as {
      Deno?: { env?: { get?: (k: string) => string | undefined } };
      process?: { env?: Record<string, string | undefined> };
    };
    const denoValue = g.Deno?.env?.get?.(name);
    if (denoValue) return denoValue;
    return g.process?.env?.[name];
  };

  const apiKey = getEnv('API_KEY') || getEnv('DASHSCOPE_API_KEY') || getEnv('NETLIFY_API_KEY');
  if (!apiKey) {
    return jsonResponse({ error: 'Server API key is not configured' }, 500);
  }

  let payload: unknown;
  try {
    payload = await request.json();
  } catch (error) {
    return jsonResponse({ error: 'Invalid JSON body' }, 400);
  }

  const { model, messages, temperature } =
    (payload as { model?: string; messages?: any[]; temperature?: number }) || {};
  if (!model || !Array.isArray(messages)) {
    return jsonResponse({ error: 'Missing model or messages' }, 400);
  }

  const externalResponse = await fetch(API_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${apiKey}`,
    },
    body: JSON.stringify({
      model,
      messages,
      stream: true,
      // temperature 仅在调用方显式传入时下发，避免对不支持该参数的模型报错
      ...(typeof temperature === 'number' ? { temperature } : {}),
      max_tokens: 1500,
      enable_thinking: false,
    }),
  });

  if (!externalResponse.ok || !externalResponse.body) {
    const text = await externalResponse.text().catch(() => '');
    let message = 'AI provider request failed';
    try {
      const data = JSON.parse(text);
      message = data?.error?.message || data?.message || message;
    } catch {
      // 忽略解析失败
    }
    return jsonResponse({ error: message }, externalResponse.status || 502);
  }

  // 透传 SSE 流给前端，边缘函数不再阻塞等待完整结果
  return new Response(externalResponse.body, {
    status: 200,
    headers: {
      'Content-Type': 'text/event-stream; charset=utf-8',
      'Cache-Control': 'no-cache',
      Connection: 'keep-alive',
    },
  });
};
