const API_URL = 'https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions';

export default async (request: Request) => {
  if (request.method !== 'POST') {
    return new Response(JSON.stringify({ error: 'Method Not Allowed' }), {
      status: 405,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  const apiKey = process.env.API_KEY || process.env.DASHSCOPE_API_KEY || process.env.NETLIFY_API_KEY;
  if (!apiKey) {
    return new Response(JSON.stringify({ error: 'Server API key is not configured' }), {
      status: 500,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  let payload: unknown;
  try {
    payload = await request.json();
  } catch (error) {
    return new Response(JSON.stringify({ error: 'Invalid JSON body' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  const { model, messages } = (payload as { model?: string; messages?: any[] }) || {};
  if (!model || !Array.isArray(messages)) {
    return new Response(JSON.stringify({ error: 'Missing model or messages' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json' },
    });
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
      temperature: 0.8,
      max_tokens: 1500,
      enable_thinking: false,
    }),
  });

  const responseText = await externalResponse.text();
  let responseData: any;
  try {
    responseData = JSON.parse(responseText);
  } catch {
    return new Response(JSON.stringify({ error: 'Invalid response from AI provider' }), {
      status: 502,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  if (!externalResponse.ok) {
    return new Response(JSON.stringify({ error: responseData.error?.message || responseData.message || 'AI provider request failed' }), {
      status: externalResponse.status,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  const content = responseData?.choices?.[0]?.message?.content || responseData?.choices?.[0]?.text || responseData?.content || '';
  if (!content) {
    return new Response(JSON.stringify({ error: 'AI provider returned no content' }), {
      status: 502,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  return new Response(JSON.stringify({ content, meta: responseData }), {
    headers: { 'Content-Type': 'application/json' },
  });
};
