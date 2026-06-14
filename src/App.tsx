import { useEffect } from 'react';
import { ChatInterface } from '@/components/ChatInterface';
import { useChat } from '@/hooks/useChat';
import { useSettings } from '@/hooks/useSettings';
import { Toaster } from '@/components/ui/sonner';
import './App.css';

function App() {
  const { 
    settings, 
    isLoaded, 
    updateModel, 
    updateUserAvatar, 
    toggleDarkMode, 
    setDarkMode 
  } = useSettings();
  
  const { 
    messages, 
    isLoading, 
    streamingContent,
    streamingMessageId,
    error, 
    sendMessage, 
    clearMessages, 
    regenerateResponse 
  } = useChat(settings.model);

  // Apply dark/light mode to document
  useEffect(() => {
    if (isLoaded) {
      if (settings.isDarkMode) {
        document.documentElement.classList.remove('light');
      } else {
        document.documentElement.classList.add('light');
      }
    }
  }, [settings.isDarkMode, isLoaded]);

  // Prevent context menu on images
  useEffect(() => {
    const handleContextMenu = (e: MouseEvent) => {
      if ((e.target as HTMLElement).tagName === 'IMG') {
        e.preventDefault();
      }
    };
    
    document.addEventListener('contextmenu', handleContextMenu);
    return () => document.removeEventListener('contextmenu', handleContextMenu);
  }, []);

  if (!isLoaded) {
    return (
      <div className="w-full h-screen flex items-center justify-center bg-background">
        <div className="flex flex-col items-center gap-4">
          <div className="w-16 h-16 rounded-full border-4 border-[#F38181]/30 border-t-[#F38181] animate-spin" />
          <p className="text-muted-foreground animate-pulse">加载中...</p>
        </div>
      </div>
    );
  }

  return (
    <>
      {/* Noise Overlay */}
      <div className="noise-overlay" />
      
      {/* Main App */}
      <ChatInterface
        messages={messages}
        isLoading={isLoading}
        streamingContent={streamingContent}
        streamingMessageId={streamingMessageId}
        error={error}
        settings={settings}
        onSendMessage={sendMessage}
        onClearMessages={clearMessages}
        onRegenerateResponse={regenerateResponse}
        onUpdateModel={updateModel}
        onUpdateUserAvatar={updateUserAvatar}
        onToggleDarkMode={toggleDarkMode}
        onSetDarkMode={setDarkMode}
      />
      
      {/* Toast Notifications */}
      <Toaster 
        position="top-center"
        toastOptions={{
          style: {
            background: 'var(--card)',
            color: 'var(--card-foreground)',
            border: '1px solid var(--border)'
          }
        }}
      />
    </>
  );
}

export default App;
