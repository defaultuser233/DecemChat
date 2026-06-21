import path from "path"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"
import { inspectAttr } from 'kimi-plugin-inspect-react'

// https://vite.dev/config/
export default defineConfig({
  base: './',
  plugins: [inspectAttr(), react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) {
            return undefined;
          }

          const normalizedId = id.replace(/\\/g, '/');
          const parts = normalizedId.split(/node_modules\//);
          const pkgPath = parts[parts.length - 1];
          if (!pkgPath) {
            return undefined;
          }

          const pkgName = pkgPath.startsWith('@')
            ? pkgPath.split('/').slice(0, 2).join('/')
            : pkgPath.split('/')[0];

          if (pkgName === 'react' || pkgName === 'react-dom' || pkgName === 'react-is') {
            return 'react-vendor';
          }

          if (pkgName.startsWith('@radix-ui')) {
            return 'radix-vendor';
          }

          if (pkgName === 'lucide-react') {
            return 'icons-vendor';
          }

          if (pkgName === 'sonner') {
            return 'toast-vendor';
          }

          if (pkgName === 'react-markdown') {
            return 'react-markdown-vendor';
          }

          if (pkgName === 'highlight.js') {
            return 'highlight-vendor';
          }

          if (pkgName.startsWith('rehype') || pkgName.startsWith('remark')) {
            return 'md-plugins-vendor';
          }

          if (
            pkgName === 'recharts' ||
            pkgName === 'react-day-picker' ||
            pkgName === 'date-fns' ||
            pkgName === 'embla-carousel-react'
          ) {
            return 'charts-vendor';
          }

          return undefined;
        },
      },
    },
  },
});
