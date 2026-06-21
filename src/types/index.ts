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
  charAvatar: string;
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
  // 千问模型 - 速度最快，免费额度，支持视觉理解 (1M上下文)
  {
    id: 'qwen3.6-flash',
    name: 'Qwen3.6-Flash',
    description: '⚡像闪电一样快的小狐狸！适合需要快速响应的日常对话，虽然体积小但能力不弱哦～',
    supportsVision: true,
    contextWindow: 1048576
  },
  // 千问模型 - 平衡性能，支持视觉理解 (1M上下文)
  {
    id: 'qwen3.6-plus',
    name: 'Qwen3.6-Plus',
    description: '🧠 更聪明的大狐狸！推理和复杂任务更强，像能同时记住浆果藏在森林的哪里、又帮松鼠算松子库存～',
    supportsVision: true,
    contextWindow: 1048576
  },
  // 千问模型 - 最强能力 (1M上下文)
  {
    id: 'qwen3.7-max',
    name: 'Qwen3.7-Max',
    description: '🏆森林里的智者！综合能力最强，能写诗、解谜、画地图，还会用尾巴尖编复杂的故事～',
    contextWindow: 1048576
  },
  // 千问模型 - 强能力+视觉理解 (1M上下文)
  {
    id: 'qwen3.7-plus',
    name: 'Qwen3.7-Plus',
    description: '🔭擅长观察的狐狸天文学家！推理和视觉理解都很强，能用望远镜看懂星空照片～',
    supportsVision: true,
    contextWindow: 1048576
  },
  // 千问全模态模型 - 支持视觉理解 (1M上下文)
  {
    id: 'qwen3.5-omni-plus',
    name: 'Qwen3.5-Omni-Plus',
    description: '🌐全能的旅行狐狸！多语言和跨领域知识都擅长，像会翻译鸟语、看懂蘑菇生长的密码～',
    supportsVision: true,
    contextWindow: 1048576
  },
  // DeepSeek 模型 (1M上下文)
  {
    id: 'deepseek-v4-pro',
    name: 'DeepSeek-V4-Pro',
    description: '🐳人坏，小鲸鱼好',
    contextWindow: 1048576
  },
  {
    id: 'deepseek-v4-flash',
    name: 'DeepSeek-V4-Flash',
    description: '🐳小鲸鱼最好了',
    contextWindow: 1048576
  },
  // 其他第三方模型 (超长上下文)
  {
    id: 'kimi-k2.6',
    name: 'Kimi-K2.6',
    description: '🌕寻求将能源转化为智能的最优解',
    supportsVision: true,
    contextWindow: 262144
  },
  {
    id: 'glm-5.1',
    name: 'GLM-5.1',
    description: '🐼整理竹子的熊猫',
    contextWindow: 262144
  }
];

export const CHAR_AVATARS = [
  './images/char_01.jpg',
  './images/char_02.jpg',
  './images/char_03.jpg'
];

export const USER_AVATARS = [
  './images/user_01.jpg',
  './images/user_02.jpg'
];

export const getRandomCharAvatar = () => {
  const index = Math.floor(Math.random() * CHAR_AVATARS.length);
  return CHAR_AVATARS[index];
};

export const getRandomUserAvatar = () => {
  const index = Math.floor(Math.random() * USER_AVATARS.length);
  return USER_AVATARS[index];
};

export const DEFAULT_SETTINGS: ChatSettings = {
  model: 'qwen3.6-flash',
  charAvatar: getRandomCharAvatar(),
  userAvatar: getRandomUserAvatar(),
  isDarkMode: true,
  temperature: 0.8
};
