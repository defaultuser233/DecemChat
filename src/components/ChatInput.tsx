import { useState, useRef, useCallback } from 'react';
import { Send, Image, X } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface ChatInputProps {
  onSendMessage: (content: string, imageUrl?: string) => void;
  isLoading: boolean;
}

export function ChatInput({ onSendMessage, isLoading }: ChatInputProps) {
  const [input, setInput] = useState('');
  const [imagePreview, setImagePreview] = useState<string | null>(null);
  const [isFocused, setIsFocused] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const imageInputRef = useRef<HTMLInputElement>(null);

  // Auto-resize textarea
  const adjustTextareaHeight = useCallback(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(textarea.scrollHeight, 120)}px`;
    }
  }, []);

  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
    adjustTextareaHeight();
  };

  const handleSend = useCallback(() => {
    if ((!input.trim() && !imagePreview) || isLoading) return;
    
    onSendMessage(input.trim(), imagePreview || undefined);
    setInput('');
    setImagePreview(null);
    
    // Reset textarea height
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
    }
  }, [input, imagePreview, isLoading, onSendMessage]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleImageSelect = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      // Check file size (max 5MB)
      if (file.size > 5 * 1024 * 1024) {
        alert('图片大小不能超过5MB');
        return;
      }
      
      const reader = new FileReader();
      reader.onloadend = () => {
        setImagePreview(reader.result as string);
      };
      reader.readAsDataURL(file);
    }
  }, []);

  const clearImage = useCallback(() => {
    setImagePreview(null);
    if (imageInputRef.current) {
      imageInputRef.current.value = '';
    }
  }, []);

  return (
    <div className="border-t border-border bg-card/80 backdrop-blur-sm p-3">
      {/* Image Preview */}
      {imagePreview && (
        <div className="mb-3 animate-slide-up">
          <div className="relative inline-block">
            <img
              src={imagePreview}
              alt="Preview"
              className="h-20 rounded-lg object-cover border border-border"
            />
            <button
              onClick={clearImage}
              className="absolute -top-2 -right-2 w-6 h-6 bg-destructive text-white rounded-full flex items-center justify-center hover:bg-destructive/80 transition-colors"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
        </div>
      )}

      {/* Input Area */}
      <div 
        className={`flex items-end gap-2 bg-muted/50 rounded-2xl p-2 transition-all duration-300 ${
          isFocused ? 'ring-2 ring-[#F38181]/30' : ''
        }`}
      >
        {/* Image Upload Button */}
        <input
          ref={imageInputRef}
          type="file"
          accept="image/*"
          onChange={handleImageSelect}
          className="hidden"
        />
        <Button
          variant="ghost"
          size="icon"
          onClick={() => imageInputRef.current?.click()}
          disabled={isLoading || !!imagePreview}
          className="flex-shrink-0 w-10 h-10 rounded-xl hover:bg-[#F38181]/10 hover:text-[#F38181] transition-colors"
        >
          <Image className="w-5 h-5" />
        </Button>

        {/* Text Input */}
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleInputChange}
          onKeyDown={handleKeyDown}
          onFocus={() => setIsFocused(true)}
          onBlur={() => setIsFocused(false)}
          placeholder={imagePreview ? '添加描述（可选）...' : '发个消息聊聊呗~'}
          disabled={isLoading}
          rows={1}
          className="flex-1 bg-transparent border-none resize-none py-2.5 px-2 text-sm placeholder:text-muted-foreground focus:outline-none min-h-[40px] max-h-[120px]"
        />

        {/* Send Button */}
        <Button
          onClick={handleSend}
          disabled={(!input.trim() && !imagePreview) || isLoading}
          size="icon"
          className={`flex-shrink-0 w-10 h-10 rounded-xl transition-all duration-300 ${
            (input.trim() || imagePreview) && !isLoading
              ? 'bg-[#F38181] hover:bg-[#e86a6a] text-white'
              : 'bg-muted text-muted-foreground'
          }`}
        >
          {isLoading ? (
            <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
          ) : (
            <Send className="w-4 h-4" />
          )}
        </Button>
      </div>

      {/* Hint */}
      <div className="text-center mt-2">
        <span className="text-xs text-muted-foreground">
          按 Enter 发送，Shift + Enter 换行
        </span>
      </div>
    </div>
  );
}
