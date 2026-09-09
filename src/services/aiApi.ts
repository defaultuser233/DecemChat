import type { Message } from '@/types';
import { AVAILABLE_MODELS } from '@/types';
import { SYSTEM_PROMPT } from './SYSTEM_PROMPT';

// 调用 Netlify Edge Function 代理接口
const API_URL = '/api/chat';

// 图片消息没有文字描述时使用的默认提示
const DEFAULT_IMAGE_PROMPT = '看看这张图片～';

// 服务器未返回有效内容时的错误信息
export const NO_CONTENT_ERROR = '服务器未返回有效内容';

export interface StreamCallbacks {
  onChunk: (chunk: string) => void;
  onComplete: (fullContent: string) => void;
  onError: (error: string) => void;
}

// Check if model supports vision by looking up in AVAILABLE_MODELS
function isVisionModel(model: string): boolean {
  const modelInfo = AVAILABLE_MODELS.find(m => m.id === model);
  return modelInfo?.supportsVision ?? false;
}

// Check if model supports temperature parameter (defaults to true)
function supportsTemperature(model: string): boolean {
  const modelInfo = AVAILABLE_MODELS.find(m => m.id === model);
  return modelInfo?.supportsTemperature ?? true;
}

// Format messages for API
function formatMessages(messages: Message[], model: string): any[] {
  const formattedMessages: any[] = [
    {
      role: 'system',
      content: SYSTEM_PROMPT
    }
  ];

  // Add recent messages (last 10)
  const recentMessages = messages.slice(-10);
  
  for (const msg of recentMessages) {
    // Handle messages with images for vision models
    if (msg.imageUrl && isVisionModel(model)) {
      const textParts = [msg.content || DEFAULT_IMAGE_PROMPT];
      if (msg.hint) textParts.push(msg.hint);
      formattedMessages.push({
        role: msg.role,
        content: [
          {
            type: 'text',
            text: textParts.join('\n')
          },
          {
            type: 'image_url',
            image_url: {
              url: msg.imageUrl
            }
          }
        ]
      });
    } else if (msg.imageUrl) {
      // For non-vision models, include image reference in text
      const text = `${msg.content || DEFAULT_IMAGE_PROMPT}${msg.hint ? `\n${msg.hint}` : ''}`;
      formattedMessages.push({
        role: msg.role,
        content: `[图片] ${text}`
      });
    } else {
      formattedMessages.push({
        role: msg.role,
        content: msg.content
      });
    }
  }

  return formattedMessages;
}

// 解析 DashScope 兼容模式返回的 SSE 流
async function consumeSSE(
  body: ReadableStream<Uint8Array>,
  callbacks: StreamCallbacks
): Promise<void> {
  const reader = body.getReader();
  const decoder = new TextDecoder('utf-8');
  let buffer = '';
  let fullContent = '';
  let finished = false;

  const handleLine = (line: string) => {
    const trimmed = line.trim();
    if (!trimmed.startsWith('data:')) return;
    const data = trimmed.slice(5).trim();
    if (data === '[DONE]') {
      finished = true;
      return;
    }
    try {
      const json = JSON.parse(data);
      const delta = json?.choices?.[0]?.delta?.content;
      const full = json?.choices?.[0]?.message?.content;
      if (typeof delta === 'string' && delta) {
        fullContent += delta;
        callbacks.onChunk(delta);
      } else if (typeof full === 'string' && full) {
        fullContent = full;
      }
    } catch {
      // 忽略无法解析的行
    }
  };

  while (!finished) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() ?? '';
    for (const line of lines) {
      handleLine(line);
    }
  }
  // 处理剩余的 buffer
  buffer += decoder.decode();
  if (buffer.trim()) {
    for (const line of buffer.split('\n')) {
      handleLine(line);
    }
  }

  if (!fullContent) {
    throw new Error(NO_CONTENT_ERROR);
  }
  callbacks.onComplete(fullContent);
}

export async function sendMessageStream(
  messages: Message[],
  model: string,
  callbacks: StreamCallbacks
): Promise<void> {
  const formattedMessages = formatMessages(messages, model);

  try {
    const body: Record<string, unknown> = {
      model,
      messages: formattedMessages
    };
    // 部分模型（如 kimi-k3）不支持 temperature 参数，此时不发送
    if (supportsTemperature(model)) {
      body.temperature = 0.8;
    }

    const response = await fetch(API_URL, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify(body)
    });

    if (!response.ok) {
      const errorText = await response.text().catch(() => 'Unknown error');
      let errorMsg = `HTTP ${response.status}`;
      try {
        const parsed = JSON.parse(errorText);
        errorMsg = parsed.error || parsed.message || errorMsg;
      } catch {
        errorMsg = errorText || errorMsg;
      }
      throw new Error(errorMsg);
    }

    // 优先按 SSE 流解析，回退到 JSON
    const contentType = response.headers.get('content-type') || '';
    if (contentType.includes('text/event-stream') && response.body) {
      await consumeSSE(response.body, callbacks);
    } else {
      const data = await response.json().catch(() => null);
      const content =
        data?.content ||
        data?.choices?.[0]?.message?.content ||
        data?.choices?.[0]?.text ||
        '';
      if (!content) {
        throw new Error(NO_CONTENT_ERROR);
      }
      callbacks.onComplete(content);
    }
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : '请求失败';
    callbacks.onError(`API 请求失败: ${errorMessage}`);
  }
}

// Non-streaming version for compatibility
export async function sendMessageToAI(
  messages: Message[],
  model: string
): Promise<string> {
  return new Promise((resolve, reject) => {
    let fullContent = '';
    
    sendMessageStream(
      messages,
      model,
      {
        onChunk: (chunk) => {
          fullContent += chunk;
        },
        onComplete: (content) => {
          resolve(content);
        },
        onError: (error) => {
          reject(new Error(error));
        }
      }
    );
  });
}

export function getModelDisplayName(modelId: string): string {
  const modelMap: Record<string, string> = {
    'qwen3.8-flash': 'Qwen3.8-Flash',
    'qwen3.7-plus': 'Qwen3.7-Plus',
    'qwen3.8-max': 'Qwen3.8-Max',
    'qwen3.5-omni-plus': 'Qwen3.5-Omni-Plus',
    'deepseek-v4-pro': 'DeepSeek-V4-Pro',
    'deepseek-v4-flash': 'DeepSeek-V4-Flash',
    'kimi-k3': 'Kimi-K3',
    'glm-5.2': 'GLM-5.2',
    'MiniMax-M3': 'MiniMax-M3',
    'mimo-v2.5-pro': 'MiMo-v2.5-Pro'
  };
  return modelMap[modelId] || modelId;
}

export function supportsVision(modelId: string): boolean {
  const modelInfo = AVAILABLE_MODELS.find(m => m.id === modelId);
  return modelInfo?.supportsVision ?? false;
}


