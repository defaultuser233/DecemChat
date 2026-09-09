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

  const apiKey = process.env.API_KEY || process.env.DASHSCOPE_API_KEY || process.env.NETLIFY_API_KEY;
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
      stream: false,
      // temperature 仅在调用方显式传入时下发，避免对不支持该参数的模型报错
      ...(typeof temperature === 'number' ? { temperature } : {}),
      max_tokens: 1500,
      enable_thinking: false,
    }),
  });

  const responseText = await externalResponse.text();
  let responseData: any;
  try {
    responseData = JSON.parse(responseText);
  } catch {
    return jsonResponse({ error: 'Invalid response from AI provider' }, 502);
  }

  if (!externalResponse.ok) {
    return jsonResponse(
      { error: responseData.error?.message || responseData.message || 'AI provider request failed' },
      externalResponse.status,
    );
  }

  const content = responseData?.choices?.[0]?.message?.content || responseData?.choices?.[0]?.text || responseData?.content || '';
  if (!content) {
    return jsonResponse({ error: 'AI provider returned no content' }, 502);
  }

  return jsonResponse({ content, meta: responseData });
};
