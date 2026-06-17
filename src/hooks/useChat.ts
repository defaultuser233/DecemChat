import { useState, useCallback, useRef, useEffect } from 'react';
import type { Message } from '@/types';
import { AVAILABLE_MODELS } from '@/types';
import { sendMessageStream } from '@/services/aiApi';
import { saveImage, loadImage, clearAllImages } from '@/services/imageDB';

const CHAT_STORAGE_KEY = 'decem-chat-messages';

// 从localStorage加载聊天记录（纯文字，不含图片base64）
function loadMessagesFromStorage(): Message[] | null {
  try {
    const saved = localStorage.getItem(CHAT_STORAGE_KEY);
    if (saved) {
      const parsed: Message[] = JSON.parse(saved);
      // 恢复时移除 imageUrl（base64太占空间），保留 imagePath 标记
      return parsed.map((msg) => ({
        ...msg,
        imageUrl: undefined, // 从 IndexedDB 异步加载
      }));
    }
  } catch (error) {
    console.error('Failed to load messages:', error);
  }
  return null;
}

// 保存聊天记录到localStorage（纯文字，图片存 IndexedDB）
function saveMessagesToStorage(messages: Message[]) {
  try {
    const messagesToSave = messages.slice(-50).map((msg) => ({
      ...msg,
      imageUrl: undefined, // 不存 base64 到 localStorage
    }));
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
        content: '🦊狐狐来啦～(≧▽≦)/～💗你终于来找我玩啦！嗷呜～',
        timestamp: Date.now(),
      },
    ];
  });

  const [isLoading, setIsLoading] = useState(false);
  const [typingMessageId, setTypingMessageId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const typingTimerRef = useRef<number | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);

  // 加载完成后，异步恢复图片
  useEffect(() => {
    async function restoreImages() {
      const messagesNeedImages = messages.filter(
        (m) => m.imagePath && !m.imageUrl
      );
      if (messagesNeedImages.length === 0) return;

      const restored = await Promise.all(
        messages.map(async (msg) => {
          if (msg.imagePath && !msg.imageUrl) {
            const dataUrl = await loadImage(msg.id);
            return dataUrl ? { ...msg, imageUrl: dataUrl } : msg;
          }
          return msg;
        })
      );
      setMessages(restored);
    }
    restoreImages();
  }, []); // 只在挂载时执行一次

  // 当消息变化时保存到localStorage
  useEffect(() => {
    saveMessagesToStorage(messages);
  }, [messages]);

  const sendMessage = useCallback(
    async (content: string, imageUrl?: string) => {
      if ((!content.trim() && !imageUrl) || isLoading) return;

      if (abortControllerRef.current) {
        abortControllerRef.current.abort();
      }
      abortControllerRef.current = new AbortController();

      const messageId = Date.now().toString();
      const userMessage: Message = {
        id: messageId,
        role: 'user',
        content: content.trim(),
        timestamp: Date.now(),
        imageUrl,
        imagePath: imageUrl ? 'stored' : undefined,
      };

      // 如果有图片，先存到 IndexedDB
      if (imageUrl) {
        await saveImage(messageId, imageUrl);
      }

      const nextMessages = [...messages, userMessage];
      setMessages(nextMessages);
      setIsLoading(true);
      setError(null);

      // 创建AI消息的ID
      const assistantMessageId = (Date.now() + 1).toString();
      setTypingMessageId(assistantMessageId);

      // 添加空的AI消息占位（内容为空时显示loading）
      setMessages((prev) => [
        ...prev,
        {
          id: assistantMessageId,
          role: 'assistant',
          content: '',
          timestamp: Date.now(),
        },
      ]);

      let fullResponse = '';

      // 根据模型上下文窗口动态调整保留的历史消息数
      const modelInfo = AVAILABLE_MODELS.find(m => m.id === model);
      const historyLimit = modelInfo?.maxHistoryMessages ?? 10;

      try {
        const history = messages.slice(-historyLimit);

        await sendMessageStream(
          [...history, userMessage],
          model,
          {
            onChunk: (chunk) => {
              fullResponse += chunk;
            },
            onComplete: (responseText) => {
              fullResponse = responseText;
            },
            onError: (errorMsg) => {
              throw new Error(errorMsg);
            },
          }
        );

        if (!fullResponse) {
          throw new Error('服务器未返回有效内容');
        }

        const targetText = fullResponse;
        const typingSpeed = 8; // 每个字符的打字速度，单位毫秒
        let currentIndex = 0;

        if (typingTimerRef.current) {
          window.clearInterval(typingTimerRef.current);
        }

        setMessages((prev) => {
          const lastIdx = prev.length - 1;
          if (lastIdx >= 0 && prev[lastIdx].id === assistantMessageId) {
            const updated = [...prev];
            updated[lastIdx] = {
              ...updated[lastIdx],
              content: '',
            };
            return updated;
          }
          return prev;
        });

        typingTimerRef.current = window.setInterval(() => {
          currentIndex += 1;
          const partial = targetText.slice(0, currentIndex);
          setMessages((prev) => {
            const lastIdx = prev.length - 1;
            if (lastIdx >= 0 && prev[lastIdx].id === assistantMessageId) {
              const updated = [...prev];
              updated[lastIdx] = {
                ...updated[lastIdx],
                content: partial,
              };
              return updated;
            }
            return prev;
          });

          if (currentIndex >= targetText.length) {
            if (typingTimerRef.current) {
              window.clearInterval(typingTimerRef.current);
              typingTimerRef.current = null;
            }
            setTypingMessageId(null);
            setIsLoading(false);
          }
        }, typingSpeed);
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : '发送消息失败';
        setError(errorMessage);
        setTypingMessageId(null);
        setIsLoading(false);
        console.error('Chat error:', err);
      } finally {
        abortControllerRef.current = null;
      }
    },
    [messages, model, isLoading]
  );

  const clearMessages = useCallback(() => {
    setMessages([
      {
        id: 'welcome',
        role: 'assistant',
        content: '🦊狐狐来啦～(≧▽≦)/～💗你终于来找我玩啦！嗷呜～',
        timestamp: Date.now(),
      },
    ]);
    setError(null);
    setTypingMessageId(null);
    if (typingTimerRef.current) {
      window.clearInterval(typingTimerRef.current);
      typingTimerRef.current = null;
    }
    localStorage.removeItem(CHAT_STORAGE_KEY);
    clearAllImages().catch(console.error);
  }, []);

  const regenerateResponse = useCallback(async () => {
    const lastUserMessage = messages
      .filter((m) => m.role === 'user')
      .pop();
    if (!lastUserMessage) return;

    const lastMessage = messages[messages.length - 1];
    if (lastMessage?.role === 'assistant') {
      setMessages((prev) => prev.slice(0, -1));
    }

    setIsLoading(true);
    setError(null);

    const assistantMessageId = Date.now().toString();
    setTypingMessageId(assistantMessageId);

    setMessages((prev) => [
      ...prev,
      {
        id: assistantMessageId,
        role: 'assistant',
        content: '',
        timestamp: Date.now(),
      },
    ]);

    let fullResponse = '';

    try {
      const modelInfo = AVAILABLE_MODELS.find(m => m.id === model);
      const historyLimit = (modelInfo?.maxHistoryMessages ?? 10) + 1;
      const history = messages.slice(-historyLimit, -1);

      await sendMessageStream(
        [...history, lastUserMessage],
        model,
        {
          onChunk: (chunk) => {
            fullResponse += chunk;
          },
          onComplete: (responseText) => {
            fullResponse = responseText;
          },
          onError: (errorMsg) => {
            throw new Error(errorMsg);
          },
        }
      );

      if (!fullResponse) {
        throw new Error('服务器未返回有效内容');
      }

      const targetText = fullResponse;
      const typingSpeed = 12;
      let currentIndex = 0;

      if (typingTimerRef.current) {
        window.clearInterval(typingTimerRef.current);
      }

      setMessages((prev) => {
        const lastIdx = prev.length - 1;
        if (lastIdx >= 0 && prev[lastIdx].id === assistantMessageId) {
          const updated = [...prev];
          updated[lastIdx] = {
            ...updated[lastIdx],
            content: '',
          };
          return updated;
        }
        return prev;
      });

      typingTimerRef.current = window.setInterval(() => {
        currentIndex += 1;
        const partial = targetText.slice(0, currentIndex);
        setMessages((prev) => {
          const lastIdx = prev.length - 1;
          if (lastIdx >= 0 && prev[lastIdx].id === assistantMessageId) {
            const updated = [...prev];
            updated[lastIdx] = {
              ...updated[lastIdx],
              content: partial,
            };
            return updated;
          }
          return prev;
        });

        if (currentIndex >= targetText.length) {
          if (typingTimerRef.current) {
            window.clearInterval(typingTimerRef.current);
            typingTimerRef.current = null;
          }
          setTypingMessageId(null);
          setIsLoading(false);
        }
      }, typingSpeed);
    } catch (err) {
      const errorMessage =
        err instanceof Error ? err.message : '重新生成失败';
      setError(errorMessage);
      setTypingMessageId(null);
      setIsLoading(false);
    }
  }, [messages, model]);

  return {
    messages,
    isLoading,
    typingMessageId,
    error,
    sendMessage,
    clearMessages,
    regenerateResponse,
  };
}
