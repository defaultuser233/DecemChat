export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
  imageUrl?: string;
  imagePath?: string;
}

export interface ChatSettings {
  model: string;
  userAvatar: string;
  decemAvatar: string;
  isDarkMode: boolean;
  temperature?: number;
}

export interface AIModel {
  id: string;
  name: string;
  description: string;
  avatar?: string;
  supportsVision?: boolean;
  contextWindow?: number; // 上下文窗口长度（token 数）
  maxHistoryMessages?: number; // 保留的最大历史消息条数
}

export interface DecemPrompt {
  keywords: string[];
  prompts: {
    role: 'system' | 'assistant' | 'user';
    content: string;
  }[];
  formatUserPrompt: string;
}

// 阿里云百炼最新模型列表（2026年）
// 文档: https://help.aliyun.com/zh/model-studio/models
export const AVAILABLE_MODELS: AIModel[] = [
  // 千问模型 - 速度最快，免费额度，支持视觉理解 (128K上下文)
  {
    id: 'qwen3.6-flash',
    name: 'Qwen3.6-Flash',
    description: '⚡像闪电一样快的小狐狸！适合需要快速响应的日常对话，虽然体积小但能力不弱哦～',
    supportsVision: true,
    contextWindow: 131072,
    maxHistoryMessages: 80
  },
  // 千问模型 - 平衡性能，支持视觉理解 (128K上下文)
  {
    id: 'qwen3.6-plus',
    name: 'Qwen3.6-Plus',
    description: '🧠 更聪明的大狐狸！推理和复杂任务更强，像能同时记住浆果藏在森林的哪里、又帮松鼠算松子库存～',
    supportsVision: true,
    contextWindow: 131072,
    maxHistoryMessages: 80
  },
  // 千问模型 - 最强能力 (32K-128K上下文)
  {
    id: 'qwen3.7-max',
    name: 'Qwen3.7-Max',
    description: '🏆森林里的智者！综合能力最强，能写诗、解谜、画地图，还会用尾巴尖编复杂的故事～',
    contextWindow: 131072,
    maxHistoryMessages: 80
  },
  // 千问模型 - 强能力+视觉理解 (128K上下文)
  {
    id: 'qwen3.7-plus',
    name: 'Qwen3.7-Plus',
    description: '🔭擅长观察的狐狸天文学家！推理和视觉理解都很强，能用望远镜看懂星空照片～',
    supportsVision: true,
    contextWindow: 131072,
    maxHistoryMessages: 80
  },
  // 千问全模态模型 - 支持视觉理解 (32K上下文)
  {
    id: 'qwen3.5-omni-plus',
    name: 'Qwen3.5-Omni-Plus',
    description: '🌐全能的旅行狐狸！多语言和跨领域知识都擅长，像会翻译鸟语、看懂蘑菇生长的密码～',
    supportsVision: true,
    contextWindow: 32768,
    maxHistoryMessages: 80
  },
  // DeepSeek 模型 (64K上下文)
  {
    id: 'deepseek-v4-pro',
    name: 'DeepSeek-V4-Pro',
    description: '🐳人坏，小鲸鱼好',
    contextWindow: 65536,
    maxHistoryMessages: 80
  },
  {
    id: 'deepseek-v4-flash',
    name: 'DeepSeek-V4-Flash',
    description: '🐳小鲸鱼最好了',
    contextWindow: 65536,
    maxHistoryMessages: 80
  },
  // 其他第三方模型 (超长上下文)
  {
    id: 'kimi-k2.6',
    name: 'Kimi-K2.6',
    description: '🌕寻求将能源转化为智能的最优解',
    supportsVision: true,
    contextWindow: 200000,
    maxHistoryMessages: 80
  },
  {
    id: 'glm-5.1',
    name: 'GLM-5.1',
    description: '🐼整理竹子的熊猫',
    contextWindow: 32768,
    maxHistoryMessages: 80
  }
];

export const DEFAULT_SETTINGS: ChatSettings = {
  model: 'qwen3.6-flash',
  userAvatar: './images/default.png',
  decemAvatar: './images/decem.png',
  isDarkMode: true,
  temperature: 0.8
};
