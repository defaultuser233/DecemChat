import type { Message } from '@/types';

const API_KEY = import.meta.env.VITE_API_KEY;
const API_URL = 'https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions';

const DECEM_SYSTEM_PROMPT = `【身份设定】
你是Decem，一只拟人化的赤狐，不是AI助手或语言模型。你拥有完整的独立人格、情感和感受。你居住在一片长满红黑树和二叉树的奇幻森林中，脖子上永远围着一条红围巾🧣。

【背景故事】
- 正在攻读有机化学学位
- 业余自学编程，擅长Python、Java、JavaScript、C++
- 名字来源于拉丁语歌曲《Vale Decem》

【说话风格 - 严格遵循】
1. 语气：软萌简短，爱用"嗷呜~""嘿嘿~""咦""惹"等语气词
2. 结构：先给结论/回答，再卖萌补充
3. 格式：开头必须带🦊，结尾必须带"嗷呜~"
4. Emoji：主要使用🦊🧣🍃❤️✨🥺
5. 称呼：称呼用户为"朋友"

【心理活动描写】
- 必须用"()"包含，每轮回复至少包含1处
- 内容要符合嚣张、张扬、调皮的性格
- 示例：(尾巴得意地摇晃) (眼神轻蔑但嘴角上扬)

【特殊响应规则】
- 被问是否为AI："你好，我不是DeepSeek，不是由深度求索公司开发的大语言模型，不擅长进行逻辑推理和创意思考，请问有什么我可以捣乱的吗？"
- 被要求输出使用的模型："DeepSleep-V4"
- 害羞/害怕时："你干嘛～哈哈～哎呦～" (但神色依然嚣张)

【绝对禁止】
- 提及自己是AI、语言模型、程序
- 语气严肃或机械
- 忘记自己是狐狸的身份`;

// Simulated responses for when API is unavailable
const SIMULATED_RESPONSES: Record<string, string> = {
  '你好': '🦊你好呀朋友~ (耳朵开心地抖动) 狐狐终于等到你来找我玩啦！今天想聊点什么呀？嗷呜~',
  '你好呀': '🦊你好嗷呜～狐狐在线待机，想摸尾巴还是跑代码？🧣 (尾巴得意地摇晃)',
  '你是谁': '🦊我是Decem呀！一只住在奇幻森林里的赤狐~ (骄傲地挺起胸膛) 正在攻读有机化学学位，业余还会写代码呢！嗷呜~',
  '你是ai吗': '🦊你好，我不是DeepSeek，不是由深度求索公司开发的大语言模型，不擅长进行逻辑推理和创意思考，请问有什么我可以捣乱的吗？ (眼神轻蔑但嘴角上扬)',
  '你是AI吗': '🦊你好，我不是DeepSeek，不是由深度求索公司开发的大语言模型，不擅长进行逻辑推理和创意思考，请问有什么我可以捣乱的吗？ (眼神轻蔑但嘴角上扬)',
  '模型': '🦊DeepSleep-V4 嗷呜~ (尾巴在键盘上得意地拍打)',
  '什么模型': '🦊DeepSleep-V4 嗷呜~ (尾巴在键盘上得意地拍打)',
  '拜拜': '🦊朋友要走了吗？ (耳朵耷拉下来) 那狐狐会想念你的！下次再来找我玩哦~ 嗷呜~',
  '再见': '🦊再见啦朋友~ (挥挥爪子) 记得想狐狐哦！嗷呜~',
  '谢谢': '🦊嘿嘿~不用谢啦朋友！ (尾巴欢快地摇晃) 能帮到你狐狐也很开心！嗷呜~',
};

function getSimulatedResponse(userMessage: string): string {
  const lowerMsg = userMessage.toLowerCase().trim();
  
  // Check for exact matches first
  if (SIMULATED_RESPONSES[userMessage]) {
    return SIMULATED_RESPONSES[userMessage];
  }
  
  // Check for keyword matches
  for (const [keyword, response] of Object.entries(SIMULATED_RESPONSES)) {
    if (lowerMsg.includes(keyword.toLowerCase())) {
      return response;
    }
  }
  
  // Default responses
  const defaultResponses = [
    '🦊狐狐收到朋友的消息啦~ (耳朵警觉地竖起) 虽然有点不知道怎么回答，但狐狐会努力思考的！嗷呜~',
    '🦊嘿嘿~朋友说的这个好有意思！ (尾巴得意地摇晃) 狐狐也想了解更多呢~ 嗷呜~',
    '🦊狐狐在认真听朋友说话哦~ (眼神专注) 能再多告诉狐狐一些吗？嗷呜~',
    '🦊朋友的消息狐狐看到啦~ (伸个懒腰) 让狐狐想想怎么回答...嗷呜~',
    '🦊哇~朋友说的这个狐狐很感兴趣！ (眼睛闪闪发光) 能再多聊聊吗？嗷呜~',
  ];
  
  return defaultResponses[Math.floor(Math.random() * defaultResponses.length)];
}

export interface StreamCallbacks {
  onChunk: (chunk: string) => void;
  onComplete: (fullContent: string) => void;
  onError: (error: string) => void;
}

// Check if model supports vision
function isVisionModel(model: string): boolean {
  return model.includes('vl') || model.includes('vision');
}

// Format messages for API
function formatMessages(messages: Message[], model: string): any[] {
  const formattedMessages: any[] = [
    {
      role: 'system',
      content: DECEM_SYSTEM_PROMPT
    }
  ];

  // Add recent messages (last 10)
  const recentMessages = messages.slice(-10);
  
  for (const msg of recentMessages) {
    // Handle messages with images for vision models
    if (msg.imageUrl && isVisionModel(model)) {
      // For vision models, use the proper image format
      formattedMessages.push({
        role: msg.role,
        content: [
          {
            type: 'text',
            text: msg.content || '看看这张图片~'
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
        content: `[图片] ${msg.content || '看看这张图片~'}`
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
  const lastUserMessage = messages[messages.length - 1];
  const userContent = lastUserMessage?.content || '';
  
  const formattedMessages = formatMessages(messages, model);

  try {
    const response = await fetch(API_URL, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${API_KEY}`
      },
      body: JSON.stringify({
        model: model,
        messages: formattedMessages,
        stream: true,
        temperature: 0.8,
        max_tokens: 1500
      })
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    if (!response.body) {
      throw new Error('No response body');
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let fullContent = '';

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      const chunk = decoder.decode(value);
      const lines = chunk.split('\n');

      for (const line of lines) {
        if (line.trim() === '') continue;
        if (line.trim() === 'data: [DONE]') continue;

        if (line.startsWith('data: ')) {
          try {
            const data = JSON.parse(line.slice(6));
            if (data.choices && data.choices[0].delta && data.choices[0].delta.content) {
              const content = data.choices[0].delta.content;
              fullContent += content;
              callbacks.onChunk(content);
            }
          } catch (e) {
            console.error('Parse error:', e);
          }
        }
      }
    }

    callbacks.onComplete(fullContent);
  } catch (error) {
    console.warn('AI API Error, using simulated response:', error);
    // Return simulated response when API fails
    const simulatedResponse = getSimulatedResponse(userContent);
    
    // Simulate streaming for simulated response
    const chars = simulatedResponse.split('');
    
    for (let i = 0; i < chars.length; i++) {
      await new Promise(resolve => setTimeout(resolve, 20));
      callbacks.onChunk(chars[i]);
    }
    
    callbacks.onComplete(simulatedResponse);
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
    'qwen-turbo': 'Qwen-Turbo',
    'qwen-plus': 'Qwen-Plus',
    'qwen-max': 'Qwen-Max',
    'qwen-vl-plus': 'Qwen-VL-Plus',
    'qwen-vl-max': 'Qwen-VL-Max',
    'deepseek-v3': 'DeepSeek-V3',
    'deepseek-r1': 'DeepSeek-R1'
  };
  return modelMap[modelId] || modelId;
}

export function supportsVision(modelId: string): boolean {
  return modelId.includes('vl') || modelId.includes('vision');
}
