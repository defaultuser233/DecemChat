import { useState, useCallback, useRef, useEffect } from 'react';
import type { Message } from '@/types';
import { sendMessageStream } from '@/services/aiApi';

const CHAT_STORAGE_KEY = 'decem-chat-messages';

// 从localStorage加载聊天记录
function loadMessagesFromStorage(): Message[] | null {
  try {
    const saved = localStorage.getItem(CHAT_STORAGE_KEY);
    if (saved) {
      const parsed = JSON.parse(saved);
      return parsed.map((msg: Message) => ({
        ...msg,
        timestamp: msg.timestamp
      }));
    }
  } catch (error) {
    console.error('Failed to load messages:', error);
  }
  return null;
}

// 保存聊天记录到localStorage
function saveMessagesToStorage(messages: Message[]) {
  try {
    const messagesToSave = messages.slice(-50);
    localStorage.setItem(CHAT_STORAGE_KEY, JSON.stringify(messagesToSave));
  } catch (error) {
    console.error('Failed to save messages:', error);
  }
}

export function useChat(model: string) {
  const [messages, setMessages] = useState<Message[]>(() => {
    const saved = loadMessagesFromStorage();
    return saved || [
      {
        id: 'welcome',
        role: 'assistant',
        content: '🦊狐狐来啦~(≧▽≦)/~💗你终于来找我玩啦！嗷呜~',
        timestamp: Date.now()
      }
    ];
  });
  
  const [isLoading, setIsLoading] = useState(false);
  const [streamingContent, setStreamingContent] = useState('');
  const [error, setError] = useState<string | null>(null);
  const streamingMessageIdRef = useRef<string | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);

  // 当消息变化时保存到localStorage
  useEffect(() => {
    saveMessagesToStorage(messages);
  }, [messages]);

  const sendMessage = useCallback(async (content: string, imageUrl?: string) => {
    if ((!content.trim() && !imageUrl) || isLoading) return;

    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
    abortControllerRef.current = new AbortController();

    const userMessage: Message = {
      id: Date.now().toString(),
      role: 'user',
      content: content.trim(),
      timestamp: Date.now(),
      imageUrl,
      imagePath: imageUrl ? 'uploaded' : undefined
    };

    // 先添加用户消息
    setMessages(prev => [...prev, userMessage]);
    setIsLoading(true);
    setStreamingContent('');
    setError(null);

    // 创建临时的AI回复消息ID
    const assistantMessageId = (Date.now() + 1).toString();
    streamingMessageIdRef.current = assistantMessageId;
    
    // 添加空的AI消息到列表
    setMessages(prev => [...prev, {
      id: assistantMessageId,
      role: 'assistant',
      content: '',
      timestamp: Date.now()
    }]);
    
    let accumulatedContent = '';
    
    try {
      const history = messages.slice(-10);
      
      await sendMessageStream(
        [...history, userMessage],
        model,
        {
          onChunk: (chunk) => {
            accumulatedContent += chunk;
            setStreamingContent(accumulatedContent);
          },
          onComplete: (fullContent) => {
            // 完成后更新消息列表中的这条消息
            setMessages(prev => {
              const lastMsg = prev[prev.length - 1];
              if (lastMsg && lastMsg.id === assistantMessageId) {
                return [
                  ...prev.slice(0, -1),
                  { ...lastMsg, content: fullContent }
                ];
              }
              return prev;
            });
            setStreamingContent('');
            streamingMessageIdRef.current = null;
            setIsLoading(false);
          },
          onError: (errorMsg) => {
            setError(errorMsg);
            streamingMessageIdRef.current = null;
            setIsLoading(false);
          }
        }
      );
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : '发送消息失败';
      setError(errorMessage);
      streamingMessageIdRef.current = null;
      setIsLoading(false);
      console.error('Chat error:', err);
    } finally {
      abortControllerRef.current = null;
    }
  }, [messages, model, isLoading]);

  const clearMessages = useCallback(() => {
    setMessages([
      {
        id: 'welcome',
        role: 'assistant',
        content: '🦊狐狐来啦~(≧▽≦)/~💗你终于来找我玩啦！嗷呜~',
        timestamp: Date.now()
      }
    ]);
    setStreamingContent('');
    setError(null);
    streamingMessageIdRef.current = null;
    localStorage.removeItem(CHAT_STORAGE_KEY);
  }, []);

  const regenerateResponse = useCallback(async () => {
    const lastUserMessage = messages.filter(m => m.role === 'user').pop();
    if (!lastUserMessage) return;

    // 删除最后一条AI回复
    const lastMessage = messages[messages.length - 1];
    if (lastMessage.role === 'assistant') {
      setMessages(prev => prev.slice(0, -1));
    }

    setIsLoading(true);
    setStreamingContent('');
    setError(null);

    const assistantMessageId = Date.now().toString();
    streamingMessageIdRef.current = assistantMessageId;
    
    // 添加新的空AI消息
    setMessages(prev => [...prev, {
      id: assistantMessageId,
      role: 'assistant',
      content: '',
      timestamp: Date.now()
    }]);

    let accumulatedContent = '';

    try {
      const history = messages.slice(-11, -1);
      
      await sendMessageStream(
        [...history, lastUserMessage],
        model,
        {
          onChunk: (chunk) => {
            accumulatedContent += chunk;
            setStreamingContent(accumulatedContent);
          },
          onComplete: (fullContent) => {
            setMessages(prev => {
              const lastMsg = prev[prev.length - 1];
              if (lastMsg && lastMsg.id === assistantMessageId) {
                return [
                  ...prev.slice(0, -1),
                  { ...lastMsg, content: fullContent }
                ];
              }
              return prev;
            });
            setStreamingContent('');
            streamingMessageIdRef.current = null;
            setIsLoading(false);
          },
          onError: (errorMsg) => {
            setError(errorMsg);
            streamingMessageIdRef.current = null;
            setIsLoading(false);
          }
        }
      );
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : '重新生成失败';
      setError(errorMessage);
      streamingMessageIdRef.current = null;
      setIsLoading(false);
    }
  }, [messages, model]);

  return {
    messages,
    isLoading,
    streamingContent,
    streamingMessageId: streamingMessageIdRef.current,
    error,
    sendMessage,
    clearMessages,
    regenerateResponse
  };
}
