import 'tsconfig-paths/register';

const API_URL = 'https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions';

export const handler = async (event: any) => {
  if (event.httpMethod !== 'POST') {
    return {
      statusCode: 405,
      body: JSON.stringify({ error: 'Method Not Allowed' }),
    };
  }

  const apiKey = process.env.API_KEY || process.env.DASHSCOPE_API_KEY || process.env.NETLIFY_API_KEY;
  if (!apiKey) {
    return {
      statusCode: 500,
      body: JSON.stringify({ error: 'Server API key is not configured' }),
    };
  }

  if (!event.body) {
    return {
      statusCode: 400,
      body: JSON.stringify({ error: 'Request body is required' }),
    };
  }

  let payload: { model?: string; messages?: any[] };
  try {
    payload = JSON.parse(event.body);
  } catch (error) {
    return {
      statusCode: 400,
      body: JSON.stringify({ error: 'Invalid JSON body' }),
    };
  }

  const { model, messages } = payload;
  if (!model || !Array.isArray(messages)) {
    return {
      statusCode: 400,
      body: JSON.stringify({ error: 'Missing model or messages' }),
    };
  }

  try {
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
      return {
        statusCode: 502,
        body: JSON.stringify({ error: 'Invalid response from AI provider' }),
      };
    }

    if (!externalResponse.ok) {
      return {
        statusCode: externalResponse.status,
        body: JSON.stringify({ error: responseData.error?.message || responseData.message || 'AI provider request failed' }),
      };
    }

    const content = responseData?.choices?.[0]?.message?.content || responseData?.choices?.[0]?.text || responseData?.content || '';
    if (!content) {
      return {
        statusCode: 502,
        body: JSON.stringify({ error: 'AI provider returned no content' }),
      };
    }

    return {
      statusCode: 200,
      body: JSON.stringify({ content, meta: responseData }),
    };
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : 'Server proxy request failed';
    return {
      statusCode: 500,
      body: JSON.stringify({ error: message }),
    };
  }
};
