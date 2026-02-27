import { useState, useRef, useCallback } from 'react';
import { 
  X, 
  Moon, 
  Sun, 
  Trash2, 
  User, 
  Bot, 
  Upload,
  AlertTriangle
} from 'lucide-react';
import type { ChatSettings } from '@/types';
import { AVAILABLE_MODELS } from '@/types';
import { getModelDisplayName, supportsVision } from '@/services/aiApi';
import { Eye } from 'lucide-react';
import { Switch } from '@/components/ui/switch';
import { Button } from '@/components/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';

interface SettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
  settings: ChatSettings;
  onUpdateModel: (model: string) => void;
  onUpdateUserAvatar: (avatar: string) => void;
  onToggleDarkMode: () => void;
  onSetDarkMode: (isDark: boolean) => void;
  onClearChat: () => void;
}

export function SettingsPanel({
  isOpen,
  onClose,
  settings,
  onUpdateModel,
  onUpdateUserAvatar,
  onSetDarkMode,
  onClearChat
}: SettingsPanelProps) {
  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const userAvatarInputRef = useRef<HTMLInputElement>(null);

  const handleUserAvatarUpload = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onloadend = () => {
        onUpdateUserAvatar(reader.result as string);
      };
      reader.readAsDataURL(file);
    }
  }, [onUpdateUserAvatar]);

  const handleClearChat = useCallback(() => {
    onClearChat();
    setShowClearConfirm(false);
  }, [onClearChat]);

  if (!isOpen) return null;

  return (
    <>
      {/* Mask Overlay */}
      <div 
        className="fixed inset-0 mask-overlay z-40 animate-fade-in"
        onClick={onClose}
      />

      {/* Drawer */}
      <div className="fixed right-0 top-0 h-full w-80 glass-effect z-50 animate-slide-in-right shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-border">
          <h2 className="text-lg font-semibold font-['Poppins']">设置</h2>
          <button
            onClick={onClose}
            className="p-2 rounded-lg hover:bg-muted transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 space-y-6 overflow-y-auto h-[calc(100%-70px)] custom-scrollbar">
          {/* Model Selection */}
          <div className="space-y-3">
            <label className="text-sm font-medium flex items-center gap-2">
              <Bot className="w-4 h-4 text-[#F38181]" />
              切换模型
            </label>
            <Select value={settings.model} onValueChange={onUpdateModel}>
              <SelectTrigger className="w-full bg-muted/50">
                <SelectValue placeholder="选择模型" />
              </SelectTrigger>
              <SelectContent>
                {AVAILABLE_MODELS.map((model) => (
                  <SelectItem key={model.id} value={model.id}>
                    <div className="flex items-center gap-2">
                      <div className="flex flex-col items-start">
                        <span className="flex items-center gap-1">
                          {model.name}
                          {supportsVision(model.id) && (
                            <Eye className="w-3 h-3 text-[#95E1D3]" />
                          )}
                        </span>
                        <span className="text-xs text-muted-foreground">{model.description}</span>
                      </div>
                    </div>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground flex items-center gap-1">
              当前: {getModelDisplayName(settings.model)}
              {supportsVision(settings.model) && (
                <span className="flex items-center gap-0.5 text-[#95E1D3]">
                  <Eye className="w-3 h-3" />
                  支持图片
                </span>
              )}
            </p>
          </div>

          {/* Divider */}
          <div className="h-px bg-border" />

          {/* User Avatar */}
          <div className="space-y-3">
            <label className="text-sm font-medium flex items-center gap-2">
              <User className="w-4 h-4 text-[#95E1D3]" />
              更改头像
            </label>
            <div className="flex items-center gap-4">
              <img
                src={settings.userAvatar}
                alt="用户头像"
                className="w-16 h-16 rounded-full object-cover border-2 border-[#F38181]"
              />
              <div className="flex-1">
                <input
                  ref={userAvatarInputRef}
                  type="file"
                  accept="image/*"
                  onChange={handleUserAvatarUpload}
                  className="hidden"
                />
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => userAvatarInputRef.current?.click()}
                  className="w-full"
                >
                  <Upload className="w-4 h-4 mr-2" />
                  上传头像
                </Button>
              </div>
            </div>
          </div>

          {/* Divider */}
          <div className="h-px bg-border" />

          {/* Dark Mode Toggle */}
          <div className="space-y-3">
            <label className="text-sm font-medium flex items-center gap-2">
              {settings.isDarkMode ? (
                <Moon className="w-4 h-4 text-[#FCE38A]" />
              ) : (
                <Sun className="w-4 h-4 text-[#FCE38A]" />
              )}
              深色/浅色模式
            </label>
            <div className="flex items-center justify-between p-3 bg-muted/50 rounded-lg">
              <span className="text-sm">
                {settings.isDarkMode ? '深色模式' : '浅色模式'}
              </span>
              <Switch
                checked={settings.isDarkMode}
                onCheckedChange={() => onSetDarkMode(!settings.isDarkMode)}
              />
            </div>
          </div>

          {/* Divider */}
          <div className="h-px bg-border" />

          {/* Clear Chat */}
          <div className="space-y-3">
            <label className="text-sm font-medium flex items-center gap-2 text-destructive">
              <Trash2 className="w-4 h-4" />
              清空聊天记录
            </label>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => setShowClearConfirm(true)}
              className="w-full"
            >
              <Trash2 className="w-4 h-4 mr-2" />
              清空记录
            </Button>
            <p className="text-xs text-muted-foreground">
              此操作不可撤销，将删除所有聊天记录
            </p>
          </div>

          {/* Footer Info */}
          <div className="pt-4 mt-4 border-t border-border">
            <p className="text-xs text-center text-muted-foreground">
              Decem Chat v1.0
            </p>
            <p className="text-xs text-center text-muted-foreground mt-1">
              与狐狐的奇妙对话
            </p>
          </div>
        </div>
      </div>

      {/* Clear Confirmation Dialog */}
      <AlertDialog open={showClearConfirm} onOpenChange={setShowClearConfirm}>
        <AlertDialogContent className="glass-effect">
          <AlertDialogHeader>
            <AlertDialogTitle className="flex items-center gap-2">
              <AlertTriangle className="w-5 h-5 text-destructive" />
              确认清空聊天记录？
            </AlertDialogTitle>
            <AlertDialogDescription>
              此操作将永久删除所有聊天记录，包括与Decem的对话历史。此操作不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction 
              onClick={handleClearChat}
              className="bg-destructive hover:bg-destructive/90"
            >
              确认清空
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <style>{`
        @keyframes slide-in-right {
          from {
            transform: translateX(100%);
          }
          to {
            transform: translateX(0);
          }
        }
        
        .animate-slide-in-right {
          animation: slide-in-right 0.5s cubic-bezier(0.16, 1, 0.3, 1) forwards;
        }
      `}</style>
    </>
  );
}
