
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import rehypeRaw from 'rehype-raw';
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
  isLastAssistantMessage,
}: MessageBubbleProps) {
  const isUser = message.role === 'user';

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const isInitialLoading = isStreaming && !message.content;

  // 流式消息直接显示文字，不加逐字动画（流式速度太快动画跟不上）
  const streamingText = isStreaming ? message.content : null;

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
      <div
        className={`flex flex-col ${
          isUser ? 'items-end' : 'items-start'
        } max-w-[75%]`}
      >
        {/* Bubble */}
        <div
          className={`relative px-4 py-3 ${
            isUser ? 'message-bubble-user' : 'message-bubble-bot'
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
              {isStreaming && streamingText !== null ? (
                <div className="whitespace-pre-wrap font-['Lato',sans-serif]">
                  {streamingText}
                  {/* 块光标 */}
                  <span className="inline-block w-[7px] h-[15px] ml-0.5 bg-[#F38181] animate-cursor-blink align-middle" />
                </div>
              ) : (
                <div className="prose prose-sm dark:prose-invert max-w-none markdown-body">
                  <Markdown
                    remarkPlugins={[remarkGfm]}
                    rehypePlugins={[rehypeHighlight, rehypeRaw]}
                    components={{
                      p: ({ children }) => (
                        <p className="m-0 mb-1 last:mb-0">{children}</p>
                      ),
                      pre: ({ children, className }) => (
                        <pre className={`${className || ''} bg-black/30 rounded-lg p-3 my-2 overflow-x-auto text-xs`}>
                          {children}
                        </pre>
                      ),
                      code: ({
                        inline,
                        children,
                        className,
                        ...props
                      }: {
                        inline?: boolean;
                        children?: React.ReactNode;
                        className?: string;
                      }) =>
                        inline ? (
                          <code
                            className="bg-black/20 px-1.5 py-0.5 rounded text-xs"
                            {...props}
                          >
                            {children}
                          </code>
                        ) : (
                          <code className={`text-xs ${className || ''}`} {...props}>
                            {children}
                          </code>
                        ),
                      ul: ({ children }) => (
                        <ul className="list-disc pl-5 my-1">{children}</ul>
                      ),
                      ol: ({ children }) => (
                        <ol className="list-decimal pl-5 my-1">{children}</ol>
                      ),
                      li: ({ children }) => (
                        <li className="mb-0.5">{children}</li>
                      ),
                      blockquote: ({ children }) => (
                        <blockquote className="border-l-4 border-[#F38181] pl-3 my-2 italic text-muted-foreground">
                          {children}
                        </blockquote>
                      ),
                      h1: ({ children }) => (
                        <h1 className="text-lg font-bold my-2">{children}</h1>
                      ),
                      h2: ({ children }) => (
                        <h2 className="text-base font-bold my-2">
                          {children}
                        </h2>
                      ),
                      h3: ({ children }) => (
                        <h3 className="text-sm font-bold my-1">
                          {children}
                        </h3>
                      ),
                      a: ({ children, href }) => (
                        <a
                          href={href}
                          className="text-[#F38181] hover:underline"
                          target="_blank"
                          rel="noopener noreferrer"
                        >
                          {children}
                        </a>
                      ),
                      table: ({ children }) => (
                        <table className="w-full text-xs my-2 border-collapse border border-border">
                          {children}
                        </table>
                      ),
                      th: ({ children }) => (
                        <th className="border border-border px-2 py-1 bg-muted">
                          {children}
                        </th>
                      ),
                      td: ({ children }) => (
                        <td className="border border-border px-2 py-1">
                          {children}
                        </td>
                      ),
                    }}
                  >
                    {message.content}
                  </Markdown>
                </div>
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
            {!isUser &&
              isLastAssistantMessage &&
              onRegenerate &&
              !isStreaming && (
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

      {/* Cursor blink animation */}
      <style>{`
        @keyframes cursorBlink {
          0%, 100% { opacity: 1; }
          50% { opacity: 0; }
        }

        .animate-cursor-blink {
          animation: cursorBlink 0.8s step-end infinite;
        }

        /* Markdown body styles */
        .markdown-body pre {
          background: rgba(0, 0, 0, 0.3);
        }
        .light .markdown-body pre {
          background: rgba(0, 0, 0, 0.05);
        }
        .markdown-body code {
          font-family: 'Fira Code', 'Consolas', monospace;
        }
        .markdown-body hr {
          border-color: var(--border);
          margin: 0.5rem 0;
        }
      `}</style>
    </div>
  );
}
