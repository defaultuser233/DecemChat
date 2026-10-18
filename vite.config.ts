import path from "path"
import react from "@vitejs/plugin-react"
import { defineConfig, type PluginOption } from "vite"
import { inspectAttr } from 'kimi-plugin-inspect-react'

/**
 * 生产构建才注入的 CSP。
 *
 * 不放进 index.html 的静态标记里，是因为开发时 React Refresh 会插内联脚本，
 * 严格 `script-src 'self'` 会把 HMR 直接拦死；而生产产物（Tauri 壳、VPS 上的 dist）
 * 里没有内联脚本，可以收紧。
 * Tauri 一侧另由 `src-tauri/src/server.rs` 在 HTML 响应头下发同一份策略。
 */
const CSP = [
  "default-src 'self'",
  "script-src 'self'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "connect-src 'self'",
  "font-src 'self' data:",
  "object-src 'none'",
  "base-uri 'none'",
  "frame-src 'none'",
  "form-action 'none'",
].join('; ')

function cspMeta(): PluginOption {
  return {
    name: 'decem-csp-meta',
    apply: 'build',
    transformIndexHtml(html) {
      return html.replace(
        '</title>',
        `</title>\n    <meta http-equiv="Content-Security-Policy" content="${CSP}" />`,
      )
    },
  }
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => ({
  base: './',
  // inspectAttr 会把源码路径与行号注入 DOM（生产包里能反查出 Windows 路径），只在开发时开
  plugins: [mode === 'development' ? inspectAttr() : null, react(), cspMeta()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
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

          if (pkgName === 'react-markdown' || pkgName.startsWith('rehype') || pkgName.startsWith('remark')) {
            return 'react-markdown-vendor';
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
}));
