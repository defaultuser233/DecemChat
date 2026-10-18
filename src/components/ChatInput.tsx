import { useState, useRef, useCallback } from 'react';
import { Send, Image, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { extractGifGrid } from '@/services/gifFrames';
import { compressImage } from '@/lib/images';
import { useToast } from '@/hooks/use-toast';

/** 上传大小上限：压缩前先挡掉极端文件，避免解码超大图卡住 WebView。 */
const MAX_IMAGE_BYTES = 5 * 1024 * 1024;

interface ChatInputProps {
  onSendMessage: (content: string, imageUrl?: string, hint?: string) => void;
  isLoading: boolean;
}

export function ChatInput({ onSendMessage, isLoading }: ChatInputProps) {
  const [input, setInput] = useState('');
  const [imagePreview, setImagePreview] = useState<string | null>(null);
  const [imageToSend, setImageToSend] = useState<string | null>(null);
  const [isGif, setIsGif] = useState(false);
  const [isExtracting, setIsExtracting] = useState(false);
  const [isCompressing, setIsCompressing] = useState(false);
  const [gifFrameCount, setGifFrameCount] = useState<number | null>(null);
  const [isFocused, setIsFocused] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const imageInputRef = useRef<HTMLInputElement>(null);
  // 连续选择图片时，只接受最后一次选择的结果
  const selectionTokenRef = useRef(0);
  const { toast } = useToast();
  const isProcessingImage = isExtracting || isCompressing;

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
    if ((!input.trim() && !imagePreview) || isLoading || isProcessingImage) return;

    const sendImage = imageToSend || imagePreview || undefined;
    const hint =
      sendImage && gifFrameCount !== null
        ? `[此图为GIF动图抽取的${gifFrameCount}帧拼接，按时间先后顺序排列]`
        : undefined;

    onSendMessage(input.trim(), sendImage, hint);
    setInput('');
    setImagePreview(null);
    setImageToSend(null);
    setIsGif(false);
    setGifFrameCount(null);
    setIsExtracting(false);
    setIsCompressing(false);
    selectionTokenRef.current += 1;

    // Reset textarea height and keep cursor focus
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.focus();
    }
  }, [
    input,
    imagePreview,
    imageToSend,
    gifFrameCount,
    isLoading,
    isProcessingImage,
    onSendMessage,
  ]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleImageSelect = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      // 清空 value，保证下次能重复选择同一个文件
      e.target.value = '';
      if (!file) return;

      if (file.size > MAX_IMAGE_BYTES) {
        toast({
          title: '图片过大',
          description: '图片大小不能超过5MB',
          variant: 'destructive',
        });
        return;
      }

      const token = selectionTokenRef.current + 1;
      selectionTokenRef.current = token;
      const isGifFile = file.type === 'image/gif' || /\.gif$/i.test(file.name);

      setIsGif(isGifFile);
      setGifFrameCount(null);
      setImagePreview(null);
      setImageToSend(null);
      setIsExtracting(isGifFile);
      setIsCompressing(false);

      if (isGifFile) {
        // GIF 必须原样保留（重编码会丢帧），compressImage 对 GIF 直接返回原图
        // 抽帧期间先显示原图，完成后替换为拼接网格图
        try {
          const original = await compressImage(file);
          if (selectionTokenRef.current !== token) return;
          setImagePreview(original.dataUrl);

          const result = await extractGifGrid(file);
          if (selectionTokenRef.current !== token) return;
          if (result) {
            setImagePreview(result.dataUrl);
            setImageToSend(result.dataUrl);
            setGifFrameCount(result.frameCount);
          } else {
            // 抽帧失败则回退为原图（模型只会看到首帧）
            setImageToSend(original.dataUrl);
          }
        } catch (error) {
          if (selectionTokenRef.current !== token) return;
          console.error('GIF 处理失败:', error);
          toast({
            title: '图片处理失败',
            description: '请换一张图片再试试',
            variant: 'destructive',
          });
        } finally {
          if (selectionTokenRef.current === token) setIsExtracting(false);
        }
        return;
      }

      setIsCompressing(true);
      try {
        const { dataUrl } = await compressImage(file);
        if (selectionTokenRef.current !== token) return;
        setImagePreview(dataUrl);
        setImageToSend(dataUrl);
      } catch (error) {
        if (selectionTokenRef.current !== token) return;
        console.error('图片压缩失败:', error);
        toast({
          title: '图片处理失败',
          description: '请换一张图片再试试',
          variant: 'destructive',
        });
      } finally {
        if (selectionTokenRef.current === token) setIsCompressing(false);
      }
    },
    [toast]
  );

  const clearImage = useCallback(() => {
    selectionTokenRef.current += 1;
    setImagePreview(null);
    setImageToSend(null);
    setIsGif(false);
    setGifFrameCount(null);
    setIsExtracting(false);
    setIsCompressing(false);
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
          {isGif && (
            <p className="mt-1 text-xs text-[#FCE38A]">
              {isExtracting
                ? '正在抽取 GIF 帧…'
                : gifFrameCount !== null
                  ? `GIF 已抽取 ${gifFrameCount} 帧，发送时将以拼接图识别`
                  : 'GIF 将按静态首帧发送'}
            </p>
          )}
        </div>
      )}

      {isCompressing && (
        <p className="mb-2 text-xs text-muted-foreground animate-slide-up">
          正在压缩图片…
        </p>
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
          placeholder={imagePreview ? '添加描述（可选）...' : '发个消息聊聊呗～'}
          rows={1}
          className="flex-1 bg-transparent border-none resize-none py-2.5 px-2 text-sm placeholder:text-muted-foreground focus:outline-none min-h-[40px] max-h-[120px]"
        />

        {/* Send Button */}
        <Button
          onClick={handleSend}
          disabled={(!input.trim() && !imagePreview) || isLoading || isProcessingImage}
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
