import { useRef, useEffect, useState } from 'react';
import { Settings, Sparkles } from 'lucide-react';
import type { Message } from '@/types';
import { MessageBubble } from './MessageBubble';
import { ChatInput } from './ChatInput';
import { SettingsPanel } from './SettingsPanel';
import { useToast } from '@/hooks/use-toast';

interface ChatInterfaceProps {
  messages: Message[];
  isLoading: boolean;
  streamingContent: string;
  streamingMessageId: string | null;
  error: string | null;
  settings: {
    model: string;
    userAvatar: string;
    decemAvatar: string;
    isDarkMode: boolean;
  };
  onSendMessage: (content: string, imageUrl?: string) => void;
  onClearMessages: () => void;
  onRegenerateResponse: () => void;
  onUpdateModel: (model: string) => void;
  onUpdateUserAvatar: (avatar: string) => void;
  onToggleDarkMode: () => void;
  onSetDarkMode: (isDark: boolean) => void;
}

export function ChatInterface({
  messages,
  isLoading,
  streamingContent,
  streamingMessageId,
  error,
  settings,
  onSendMessage,
  onClearMessages,
  onRegenerateResponse,
  onUpdateModel,
  onUpdateUserAvatar,
  onToggleDarkMode,
  onSetDarkMode
}: ChatInterfaceProps) {
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const { toast } = useToast();

  // Auto-scroll to bottom when new messages arrive or streaming content updates
  useEffect(() => {
    if (messagesContainerRef.current) {
      messagesContainerRef.current.scrollTop = messagesContainerRef.current.scrollHeight;
    }
  }, [messages, streamingContent]);

  // Show error toast
  useEffect(() => {
    if (error) {
      toast({
        title: '发送失败',
        description: error,
        variant: 'destructive'
      });
    }
  }, [error, toast]);

  const lastAssistantMessageIndex = [...messages]
    .reverse()
    .findIndex(m => m.role === 'assistant');
  const actualLastAssistantIndex = lastAssistantMessageIndex === -1 
    ? -1 
    : messages.length - 1 - lastAssistantMessageIndex;

  return (
    <div className="fixed inset-0 flex flex-col bg-background">
      {/* Background Effects - Fixed behind everything */}
      <div className="fixed inset-0 pointer-events-none overflow-hidden z-0">
        {/* Gradient orbs */}
        <div 
          className="absolute top-1/4 left-1/4 w-96 h-96 rounded-full opacity-20 animate-pulse-glow"
          style={{
            background: 'radial-gradient(circle, rgba(243, 129, 129, 0.4) 0%, transparent 70%)',
            filter: 'blur(60px)'
          }}
        />
        <div 
          className="absolute bottom-1/4 right-1/4 w-80 h-80 rounded-full opacity-15 animate-pulse-glow"
          style={{
            background: 'radial-gradient(circle, rgba(149, 225, 211, 0.4) 0%, transparent 70%)',
            filter: 'blur(50px)',
            animationDelay: '1.5s'
          }}
        />
        <div 
          className="absolute top-1/2 right-1/3 w-64 h-64 rounded-full opacity-10 animate-pulse-glow"
          style={{
            background: 'radial-gradient(circle, rgba(252, 227, 138, 0.4) 0%, transparent 70%)',
            filter: 'blur(40px)',
            animationDelay: '3s'
          }}
        />
      </div>

      {/* Header - Fixed at top */}
      <header className="relative z-20 flex items-center justify-between px-4 py-3 border-b border-border glass-effect shrink-0">
        <div className="flex items-center gap-3">
          {/* Decem Avatar */}
          <div className="relative">
            <img
              src={settings.decemAvatar}
              alt="Decem"
              className="w-10 h-10 rounded-full object-cover border-2 border-[#95E1D3] animate-breathe"
            />
            <div className="absolute -bottom-0.5 -right-0.5 w-3 h-3 bg-green-500 rounded-full border-2 border-background" />
          </div>
          
          {/* Title */}
          <div>
            <h1 className="text-lg font-semibold font-['Poppins'] flex items-center gap-2">
              Decem
              <Sparkles className="w-4 h-4 text-[#FCE38A]" />
            </h1>
            <p className="text-xs text-muted-foreground">
              在线 - 随时陪你聊天
            </p>
          </div>
        </div>

        {/* Settings Button */}
        <button
          onClick={() => setIsSettingsOpen(true)}
          className="p-2.5 rounded-xl hover:bg-muted transition-all duration-300 hover:rotate-90"
        >
          <Settings className="w-5 h-5" />
        </button>
      </header>

      {/* Chat Messages - Scrollable middle area */}
      <div 
        ref={messagesContainerRef}
        className="relative z-10 flex-1 overflow-y-auto custom-scrollbar px-4 py-4"
      >
        <div className="space-y-6 max-w-3xl mx-auto">
          {messages.map((message, index) => {
            const isStreaming = isLoading && message.id === streamingMessageId;
            // 如果是正在流式的消息，显示流式内容
            const displayMessage = isStreaming 
              ? { ...message, content: streamingContent }
              : message;
            
            return (
              <MessageBubble
                key={message.id}
                message={displayMessage}
                avatar={message.role === 'user' ? settings.userAvatar : settings.decemAvatar}
                isStreaming={isStreaming}
                onRegenerate={index === actualLastAssistantIndex ? onRegenerateResponse : undefined}
                isLastAssistantMessage={index === actualLastAssistantIndex}
              />
            );
          })}
        </div>
      </div>

      {/* Chat Input - Fixed at bottom */}
      <div className="relative z-20 shrink-0">
        <ChatInput 
          onSendMessage={onSendMessage}
          isLoading={isLoading}
        />
      </div>

      {/* Settings Panel */}
      <SettingsPanel
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        settings={settings}
        onUpdateModel={onUpdateModel}
        onUpdateUserAvatar={onUpdateUserAvatar}
        onToggleDarkMode={onToggleDarkMode}
        onSetDarkMode={onSetDarkMode}
        onClearChat={onClearMessages}
      />
    </div>
  );
}
