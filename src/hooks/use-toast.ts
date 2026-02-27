import { toast as sonnerToast } from 'sonner';

interface ToastOptions {
  title?: string;
  description?: string;
  variant?: 'default' | 'destructive';
  duration?: number;
}

export function useToast() {
  const toast = (options: ToastOptions) => {
    const { title, description, variant, duration } = options;
    
    if (variant === 'destructive') {
      sonnerToast.error(title, {
        description,
        duration: duration || 4000
      });
    } else {
      sonnerToast.success(title, {
        description,
        duration: duration || 4000
      });
    }
  };

  return { toast };
}
