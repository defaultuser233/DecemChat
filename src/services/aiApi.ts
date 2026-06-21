import type { Message } from '@/types';
import { AVAILABLE_MODELS } from '@/types';
import { SYSTEM_PROMPT } from './SYSTEM_PROMPT';

// 调用 Netlify Edge Function 代理接口
const API_URL = '/api/chat';

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
      formattedMessages.push({
        role: msg.role,
        content: [
          {
            type: 'text',
            text: msg.content || '看看这张图片～'
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
      formattedMessages.push({
        role: msg.role,
        content: `[图片] ${msg.content || '看看这张图片～'}`
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

export async function sendMessageStream(
  messages: Message[],
  model: string,
  callbacks: StreamCallbacks
): Promise<void> {
  const formattedMessages = formatMessages(messages, model);

  try {
    const response = await fetch(API_URL, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        model,
        messages: formattedMessages
      })
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

    const data = await response.json().catch(() => null);
    const content = data?.content || data?.choices?.[0]?.message?.content || data?.choices?.[0]?.text || '';

    if (!content) {
      throw new Error('服务器未返回有效内容');
    }

    callbacks.onComplete(content);
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
    'qwen3.6-flash': 'Qwen3.6-Flash',
    'qwen3.6-plus': 'Qwen3.6-Plus',
    'qwen3.7-max': 'Qwen3.7-Max',
    'qwen3.7-plus': 'Qwen3.7-Plus',
    'qwen3.5-omni-plus': 'Qwen3.5-Omni-Plus',
    'deepseek-v4-pro': 'DeepSeek-V4-Pro',
    'deepseek-v4-flash': 'DeepSeek-V4-Flash',
    'kimi-k2.6': 'Kimi-K2.6',
    'glm-5.1': 'GLM-5.1',
    'glm-5.2': 'GLM-5.2'
  };
  return modelMap[modelId] || modelId;
}

export function supportsVision(modelId: string): boolean {
  const modelInfo = AVAILABLE_MODELS.find(m => m.id === modelId);
  return modelInfo?.supportsVision ?? false;
}


