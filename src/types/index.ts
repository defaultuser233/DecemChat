export interface Message {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
  imageUrl?: string;
  imagePath?: string; // 存储图片路径，用于localStorage
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
}

export interface DecemPrompt {
  keywords: string[];
  prompts: {
    role: 'system' | 'assistant' | 'user';
    content: string;
  }[];
  formatUserPrompt: string;
}

export const AVAILABLE_MODELS: AIModel[] = [
  {
    id: 'qwen-turbo',
    name: 'Qwen-Turbo',
    description: '通义千问Turbo版本，响应最快'
  },
  {
    id: 'qwen-plus',
    name: 'Qwen-Plus',
    description: '通义千问Plus版本，平衡性能'
  },
  {
    id: 'qwen-max',
    name: 'Qwen-Max',
    description: '通义千问Max版本，能力最强'
  },
  {
    id: 'qwen-vl-plus',
    name: 'Qwen-VL-Plus',
    description: '通义千问视觉增强版，支持图片理解',
    supportsVision: true
  },
  {
    id: 'qwen-vl-max',
    name: 'Qwen-VL-Max',
    description: '通义千问视觉Max版，最强视觉理解',
    supportsVision: true
  },
  {
    id: 'deepseek-v3',
    name: 'DeepSeek-V3',
    description: 'DeepSeek最新版本'
  },
  {
    id: 'deepseek-r1',
    name: 'DeepSeek-R1',
    description: 'DeepSeek推理版本'
  }
];

export const DEFAULT_SETTINGS: ChatSettings = {
  model: 'qwen-turbo',
  userAvatar: '/images/default.png',
  decemAvatar: '/images/decem.png',
  isDarkMode: true,
  temperature: 0.8
};
