import { parseGIF, decompressFrames } from 'gifuct-js';
import type { ParsedGif, ParsedFrame } from 'gifuct-js';

const MAX_FRAMES = 6; // 最多抽取的帧数
const MAX_SIDE = 512; // 每帧最长边像素，避免拼接图过大

export interface GifGridResult {
  dataUrl: string;
  frameCount: number;
}

export function isGifFile(file: File): boolean {
  return file.type === 'image/gif';
}

// 按时间等间隔选取至多 maxFrames 个关键帧
function pickFrameIndicesByTime(frames: ParsedFrame[], maxFrames: number): number[] {
  const n = frames.length;
  if (n <= maxFrames) {
    return Array.from({ length: n }, (_, i) => i);
  }

  // delay 缺失或为 0 时按最小单位计，单位不影响相对采样
  const delays = frames.map((f) => (f.delay && f.delay > 0 ? f.delay : 10));
  const total = delays.reduce((sum, d) => sum + d, 0);
  if (total <= 0) {
    // 兜底：按帧序号平均
    const indices: number[] = [];
    for (let i = 0; i < maxFrames; i++) {
      indices.push(
        Math.min(n - 1, Math.floor((i * (n - 1)) / (maxFrames - 1)))
      );
    }
    return indices;
  }

  const indices: number[] = [];
  let cumulative = 0;
  let frameIdx = 0;
  for (let k = 0; k < maxFrames; k++) {
    const target = (k * total) / (maxFrames - 1);
    while (frameIdx < n - 1 && cumulative + delays[frameIdx] <= target) {
      cumulative += delays[frameIdx];
      frameIdx += 1;
    }
    if (indices[indices.length - 1] !== frameIdx) {
      indices.push(frameIdx);
    }
  }
  return indices;
}

// 网格布局：1-3 帧单行，4 帧 2×2，5-6 帧 3×2
function layoutGrid(n: number): { cols: number; rows: number } {
  if (n <= 3) return { cols: n, rows: 1 };
  if (n === 4) return { cols: 2, rows: 2 };
  return { cols: 3, rows: Math.ceil(n / 3) };
}

// 按 GIF 的 disposal 规则逐帧还原完整画面
function compositeFrames(
  gif: ParsedGif,
  frames: ParsedFrame[],
  indices: number[]
): HTMLCanvasElement[] {
  const width = gif.lsd.width;
  const height = gif.lsd.height;

  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d')!;
  ctx.clearRect(0, 0, width, height);

  const selected = new Set(indices);
  const result: HTMLCanvasElement[] = [];

  for (let i = 0; i < frames.length; i++) {
    const frame = frames[i];

    // disposalType === 3 需要还原到绘制前状态，提前保存快照
    const snapshot =
      frame.disposalType === 3 ? ctx.getImageData(0, 0, width, height) : null;

    // 用 drawImage 合成 patch（保留透明像素，让前一帧内容透出）
    if (frame.patch && frame.patch.length > 0) {
      try {
        const patchCanvas = document.createElement('canvas');
        patchCanvas.width = frame.dims.width;
        patchCanvas.height = frame.dims.height;
        const pctx = patchCanvas.getContext('2d')!;
        const imageData = pctx.createImageData(frame.dims.width, frame.dims.height);
        imageData.data.set(frame.patch);
        pctx.putImageData(imageData, 0, 0);
        ctx.drawImage(patchCanvas, frame.dims.left, frame.dims.top);
      } catch {
        // patch 尺寸异常时跳过该帧
      }
    }

    if (selected.has(i)) {
      const snap = document.createElement('canvas');
      snap.width = width;
      snap.height = height;
      snap.getContext('2d')!.drawImage(canvas, 0, 0);
      result.push(snap);
    }

    // 处理 disposal，为下一帧做准备
    if (frame.disposalType === 2) {
      ctx.clearRect(frame.dims.left, frame.dims.top, frame.dims.width, frame.dims.height);
    } else if (frame.disposalType === 3 && snapshot) {
      ctx.putImageData(snapshot, 0, 0);
    }
  }

  return result;
}

// 把多帧拼接成带序号的网格图
function combineFramesToGrid(frames: HTMLCanvasElement[]): string {
  const gap = 6;
  const labelHeight = 20;
  const { cols, rows } = layoutGrid(frames.length);

  const scaled = frames.map((f) => {
    const scale = Math.min(1, MAX_SIDE / Math.max(f.width, f.height));
    const w = Math.max(1, Math.round(f.width * scale));
    const h = Math.max(1, Math.round(f.height * scale));
    const c = document.createElement('canvas');
    c.width = w;
    c.height = h;
    const cx = c.getContext('2d')!;
    cx.fillStyle = '#ffffff';
    cx.fillRect(0, 0, w, h);
    cx.drawImage(f, 0, 0, w, h);
    return c;
  });

  const cellW = Math.max(...scaled.map((c) => c.width));
  const cellH = Math.max(...scaled.map((c) => c.height));

  const grid = document.createElement('canvas');
  grid.width = cols * cellW + (cols - 1) * gap;
  grid.height = rows * (cellH + labelHeight) + (rows - 1) * gap;
  const ctx = grid.getContext('2d')!;
  ctx.fillStyle = '#ffffff';
  ctx.fillRect(0, 0, grid.width, grid.height);

  scaled.forEach((c, i) => {
    const col = i % cols;
    const row = Math.floor(i / cols);
    const x = col * (cellW + gap);
    const y = row * (cellH + labelHeight + gap);
    ctx.drawImage(c, x, y, cellW, cellH);
    ctx.fillStyle = '#666666';
    ctx.font = '12px sans-serif';
    ctx.textBaseline = 'top';
    ctx.fillText(`帧${i + 1}`, x + 2, y + cellH + 4);
  });

  return grid.toDataURL('image/png');
}

// 从 GIF 文件抽取关键帧，返回拼接网格图；失败返回 null
export async function extractGifGrid(
  file: File,
  maxFrames = MAX_FRAMES
): Promise<GifGridResult | null> {
  if (!isGifFile(file)) return null;

  try {
    const arrayBuffer = await file.arrayBuffer();
    const gif = parseGIF(arrayBuffer);
    const frames = decompressFrames(gif, true);
    if (!frames.length) return null;

    const indices = pickFrameIndicesByTime(frames, maxFrames);
    const canvases = compositeFrames(gif, frames, indices);
    if (!canvases.length) return null;

    return {
      dataUrl: combineFramesToGrid(canvases),
      frameCount: canvases.length,
    };
  } catch (error) {
    console.error('GIF 抽帧失败:', error);
    return null;
  }
}
