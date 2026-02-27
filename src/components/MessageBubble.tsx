import type { Message } from '@/types';
import { Loader2, RotateCcw } from 'lucide-react';

interface MessageBubbleProps {
  message: Message;
  avatar: string;
  isStreaming?: boolean;
  onRegenerate?: () => void;
  isLastAssistantMessage?: boolean;
}

export function MessageBubble({
  message,
  avatar,
  isStreaming,
  onRegenerate,
  isLastAssistantMessage
}: MessageBubbleProps) {
  const isUser = message.role === 'user';

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit'
    });
  };

  // Parse content to handle markdown-like formatting
  const renderContent = (content: string) => {
    // Replace **text** with bold
    let formatted = content.replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>');
    // Replace *text* with italic
    formatted = formatted.replace(/\*(.*?)\*/g, '<em>$1</em>');
    // Replace `text` with code
    formatted = formatted.replace(/`(.*?)`/g, '<code class="bg-black/20 px-1 py-0.5 rounded text-sm">$1</code>');
    
    return { __html: formatted };
  };

  // 判断是否是初始加载状态（内容为空且正在流式）
  const isInitialLoading = isStreaming && !message.content;

  return (
    <div
      className={`flex items-start gap-3 animate-slide-up ${
        isUser ? 'flex-row-reverse' : 'flex-row'
      }`}
    >
      {/* Avatar */}
      <div className="flex-shrink-0">
        <img
          src={avatar}
          alt={isUser ? '用户' : 'Decem'}
          className={`w-10 h-10 rounded-full object-cover border-2 ${
            isUser 
              ? 'border-[#F38181]' 
              : 'border-[#95E1D3] animate-breathe'
          } avatar-hover`}
        />
      </div>

      {/* Message Content */}
      <div className={`flex flex-col ${isUser ? 'items-end' : 'items-start'} max-w-[75%]`}>
        {/* Bubble */}
        <div
          className={`relative px-4 py-3 ${
            isUser
              ? 'message-bubble-user'
              : 'message-bubble-bot'
          }`}
        >
          {/* Image if present */}
          {message.imageUrl && (
            <div className="mb-2">
              <img
                src={message.imageUrl}
                alt="Shared"
                className="max-w-full rounded-lg max-h-48 object-cover"
              />
            </div>
          )}

          {/* Text content */}
          {isInitialLoading ? (
            <div className="flex items-center gap-2">
              <Loader2 className="w-4 h-4 animate-spin" />
              <span className="text-sm">狐狐正在思考...</span>
            </div>
          ) : (
            <div className="text-sm leading-relaxed">
              {/* Content with inline cursor for streaming */}
              <span 
                dangerouslySetInnerHTML={renderContent(message.content)}
                className="whitespace-pre-wrap"
              />
              {/* Streaming cursor - inline after text */}
              {isStreaming && (
                <span className="inline-block w-[2px] h-4 bg-[#F38181] animate-pulse ml-0.5 align-middle" />
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        {!isInitialLoading && (
          <div className="flex items-center gap-2 mt-1">
            <span className="text-xs text-muted-foreground">
              {formatTime(message.timestamp)}
            </span>
            
            {/* Regenerate button for last assistant message */}
            {!isUser && isLastAssistantMessage && onRegenerate && !isStreaming && (
              <button
                onClick={onRegenerate}
                className="flex items-center gap-1 text-xs text-[#F38181] hover:text-[#e86a6a] transition-colors"
                title="重新生成"
              >
                <RotateCcw className="w-3 h-3" />
                <span>重新生成</span>
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
