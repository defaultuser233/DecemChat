import { useState, useEffect, useCallback } from 'react';
import type { ChatSettings } from '@/types';
import { DEFAULT_SETTINGS, getRandomCharAvatar, getRandomUserAvatar } from '@/types';

const STORAGE_KEY = 'decem-chat-settings';

export function useSettings() {
  const [settings, setSettings] = useState<ChatSettings>(DEFAULT_SETTINGS);
  const [isLoaded, setIsLoaded] = useState(false);

  // Load settings from localStorage on mount
  useEffect(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        // 恢复设置，但用户和 Decem 头像每次重新加载时都随机。
        setSettings({
          ...DEFAULT_SETTINGS,
          ...parsed,
          userAvatar: getRandomUserAvatar(),
          charAvatar: getRandomCharAvatar()
        });
      }
    } catch (error) {
      console.error('Failed to load settings:', error);
    }
    setIsLoaded(true);
  }, []);

  // Save settings to localStorage whenever they change
  useEffect(() => {
    if (isLoaded) {
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
      } catch (error) {
        console.error('Failed to save settings:', error);
      }
    }
  }, [settings, isLoaded]);

  const updateModel = useCallback((model: string) => {
    setSettings(prev => ({ ...prev, model }));
  }, []);

  const updateUserAvatar = useCallback((avatar: string) => {
    setSettings(prev => ({ ...prev, userAvatar: avatar }));
  }, []);

  const updateCharAvatar = useCallback((avatar: string) => {
    setSettings(prev => ({ ...prev, charAvatar: avatar }));
  }, []);

  const toggleDarkMode = useCallback(() => {
    setSettings(prev => ({ ...prev, isDarkMode: !prev.isDarkMode }));
  }, []);

  const setDarkMode = useCallback((isDark: boolean) => {
    setSettings(prev => ({ ...prev, isDarkMode: isDark }));
  }, []);

  const resetSettings = useCallback(() => {
    setSettings(DEFAULT_SETTINGS);
  }, []);

  return {
    settings,
    isLoaded,
    updateModel,
    updateUserAvatar,
    updateCharAvatar,
    toggleDarkMode,
    setDarkMode,
    resetSettings
  };
}
