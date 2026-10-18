/**
 * 图片压缩与请求体预算。
 *
 * 上传时把大图缩到最长边 1568px（视觉模型输入尺寸附近），发送前再按字节预算
 * 从最旧的消息开始降级图片，避免后端 32MiB 请求体上限触发 413。
 */

const utf8Encoder = new TextEncoder();

/** 视觉模型输入的最长边；再大也不会明显提升识别效果，只会撑大请求体。 */
const MAX_SIDE = 1568;

/** JPEG / WebP 输出质量。 */
const ENCODE_QUALITY = 0.82;

/** 低于该大小的图片跳过重编码，避免无谓的画质损失。 */
const SKIP_COMPRESSION_BYTES = 300 * 1024;

/** 单次请求的 JSON 体预算，留出余量，低于后端 32MiB 上限。 */
export const REQUEST_BUDGET_BYTES = 22 * 1024 * 1024;

/** 超预算时图片段降级成的文本。 */
export const IMAGE_PLACEHOLDER = '[图片]';

/** 截图类文件名直接按 PNG 处理：文字边缘不能有 JPEG 振铃。 */
const SCREENSHOT_NAME_PATTERN =
  /(screenshot|screen[\s_-]?shot|snip|capture|截屏|截图|スクリーンショット)/i;

/** 抽样分析用的边长；64×64 足以判断颜色复杂度，开销可以忽略。 */
const SAMPLE_SIDE = 64;

/** 每通道量化到 4 位后的颜色种类上限。照片通常 1500+，界面/文字图少得多。 */
const GRAPHIC_UNIQUE_COLORS = 1024;

/** 相邻采样点每通道差 ≤ 6 视为同一色块。截图大片纯色，照片很少。 */
const FLAT_COLOR_DELTA = 6;

/** 横向相邻采样点中「同色」的比例达到该值，按图形/截图处理。 */
const FLAT_RATIO_THRESHOLD = 0.5;

export interface CompressedImage {
  dataUrl: string;
  mime: string;
}

interface ImageAnalysis {
  hasAlpha: boolean;
  uniqueColors: number;
  flatRatio: number;
}

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error ?? new Error('读取图片失败'));
    reader.readAsDataURL(file);
  });
}

function isGifFile(file: File): boolean {
  // 部分 Android 文件选择器不带 MIME，扩展名兜底
  return file.type === 'image/gif' || /\.gif$/i.test(file.name);
}

async function decodeImage(file: File): Promise<ImageBitmap | HTMLImageElement> {
  if (typeof createImageBitmap === 'function') {
    try {
      return await createImageBitmap(file, { imageOrientation: 'from-image' });
    } catch {
      // 老 WebView 可能不支持 createImageBitmap 或忽略该选项，回退到 <img>
    }
  }

  const url = URL.createObjectURL(file);
  try {
    const image = new Image();
    image.decoding = 'async';
    await new Promise<void>((resolve, reject) => {
      image.onload = () => resolve();
      image.onerror = () => reject(new Error('图片解码失败'));
      image.src = url;
    });
    return image;
  } finally {
    URL.revokeObjectURL(url);
  }
}

/**
 * 在 64×64 采样图上统计透明度、颜色复杂度和同色相邻比例。
 * 用采样而不是全图 getImageData，避免 4K 图一次性吃掉几十 MB 内存。
 */
function analyzeImage(
  source: CanvasImageSource,
  width: number,
  height: number
): ImageAnalysis {
  const scale = Math.min(1, SAMPLE_SIDE / Math.max(width, height));
  const widthPx = Math.max(1, Math.round(width * scale));
  const heightPx = Math.max(1, Math.round(height * scale));

  const canvas = document.createElement('canvas');
  canvas.width = widthPx;
  canvas.height = heightPx;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) {
    return { hasAlpha: true, uniqueColors: GRAPHIC_UNIQUE_COLORS + 1, flatRatio: 0 };
  }
  ctx.drawImage(source, 0, 0, widthPx, heightPx);

  const { data } = ctx.getImageData(0, 0, widthPx, heightPx);
  const colors = new Set<number>();
  let hasAlpha = false;
  let flat = 0;
  let pairs = 0;

  for (let y = 0; y < heightPx; y += 1) {
    for (let x = 0; x < widthPx; x += 1) {
      const index = (y * widthPx + x) * 4;
      const r = data[index];
      const g = data[index + 1];
      const b = data[index + 2];
      if (data[index + 3] < 255) hasAlpha = true;

      colors.add(((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4));

      if (x > 0) {
        const prev = index - 4;
        pairs += 1;
        if (
          Math.abs(r - data[prev]) <= FLAT_COLOR_DELTA &&
          Math.abs(g - data[prev + 1]) <= FLAT_COLOR_DELTA &&
          Math.abs(b - data[prev + 2]) <= FLAT_COLOR_DELTA
        ) {
          flat += 1;
        }
      }
    }
  }

  return {
    hasAlpha,
    uniqueColors: colors.size,
    flatRatio: pairs > 0 ? flat / pairs : 0,
  };
}

/**
 * 截图/文字类优先 PNG：JPEG 在字符边缘产生振铃，会直接影响视觉模型的 OCR。
 * 判断依据依次是文件名、透明度、以及 PNG 源的颜色复杂度/大片纯色特征。
 */
function preferPng(file: File, analysis: ImageAnalysis): boolean {
  if (SCREENSHOT_NAME_PATTERN.test(file.name)) return true;
  if (analysis.hasAlpha) return true;
  if (file.type !== 'image/png') return false;
  return (
    analysis.uniqueColors <= GRAPHIC_UNIQUE_COLORS ||
    analysis.flatRatio >= FLAT_RATIO_THRESHOLD
  );
}

let webpSupported: boolean | null = null;

function supportsWebp(): boolean {
  if (webpSupported === null) {
    const probe = document.createElement('canvas');
    probe.width = 1;
    probe.height = 1;
    webpSupported = probe.toDataURL('image/webp').startsWith('data:image/webp');
  }
  return webpSupported;
}

/**
 * 压缩上传图片。
 *
 * - GIF 原样返回：canvas 只能保留首帧，重编码会丢动画。
 * - 小图（≤300KB）原样返回，避免无谓重编码。
 * - 其余图片缩到最长边 1568px，照片输出 JPEG/WebP（质量 0.82），
 *   截图/文字类输出 PNG。压缩后反而更大时保留原图。
 */
export async function compressImage(file: File): Promise<CompressedImage> {
  if (isGifFile(file)) {
    return {
      dataUrl: await readFileAsDataUrl(file),
      mime: file.type || 'image/gif',
    };
  }

  if (file.size <= SKIP_COMPRESSION_BYTES) {
    return {
      dataUrl: await readFileAsDataUrl(file),
      mime: file.type || 'image/jpeg',
    };
  }

  let decoded: ImageBitmap | HTMLImageElement | null = null;
  try {
    decoded = await decodeImage(file);
    const width = decoded.width;
    const height = decoded.height;
    if (!width || !height) throw new Error('图片尺寸无效');

    const analysis = analyzeImage(decoded, width, height);
    const targetMime = preferPng(file, analysis)
      ? 'image/png'
      : file.type === 'image/webp' && supportsWebp()
        ? 'image/webp'
        : 'image/jpeg';

    const scale = Math.min(1, MAX_SIDE / Math.max(width, height));
    const targetWidth = Math.max(1, Math.round(width * scale));
    const targetHeight = Math.max(1, Math.round(height * scale));

    const canvas = document.createElement('canvas');
    canvas.width = targetWidth;
    canvas.height = targetHeight;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    if (!ctx) throw new Error('无法创建 canvas 上下文');

    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = 'high';
    if (targetMime === 'image/jpeg') {
      // JPEG 没有透明通道：先铺白底，避免透明区域变黑
      ctx.fillStyle = '#ffffff';
      ctx.fillRect(0, 0, targetWidth, targetHeight);
    }
    ctx.drawImage(decoded, 0, 0, targetWidth, targetHeight);

    const dataUrl = canvas.toDataURL(targetMime, ENCODE_QUALITY);

    // 重编码反而更大时保留原图（画质也更好）
    const originalApproxLength = Math.ceil(file.size / 3) * 4 + 64;
    if (dataUrl.length >= originalApproxLength) {
      return {
        dataUrl: await readFileAsDataUrl(file),
        mime: file.type || 'image/jpeg',
      };
    }

    return { dataUrl, mime: targetMime };
  } catch (error) {
    console.warn('图片压缩失败，回退原图:', error);
    return {
      dataUrl: await readFileAsDataUrl(file),
      mime: file.type || 'image/jpeg',
    };
  } finally {
    if (
      decoded &&
      typeof ImageBitmap !== 'undefined' &&
      decoded instanceof ImageBitmap
    ) {
      decoded.close();
    }
  }
}

function jsonByteLength(value: unknown): number {
  const json = JSON.stringify(value);
  return json === undefined ? 0 : utf8Encoder.encode(json).length;
}

/** 请求体的近似字节数（UTF-8），用于和后端上限比较。纯函数。 */
export function estimateRequestBytes(body: unknown): number {
  return jsonByteLength(body);
}

export interface FitBudgetResult<T> {
  /** 处理后的请求体（就地修改并返回）。 */
  body: T;
  /** 被替换成 `[图片]` 的图片段数量。 */
  downgradedImages: number;
  /** 处理后的估算字节数。 */
  estimatedBytes: number;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

/**
 * 请求体超过预算时，按消息顺序（最旧 → 最新）把多模态图片段降级成 `[图片]`，
 * 直到达标或没有图片可降级。纯函数除返回体外不保留状态，会就地修改传入的 body。
 */
export function fitRequestBudget<T>(
  body: T,
  budgetBytes: number
): FitBudgetResult<T> {
  let estimatedBytes = estimateRequestBytes(body);
  if (estimatedBytes <= budgetBytes || !isRecord(body)) {
    return { body, downgradedImages: 0, estimatedBytes };
  }

  const messages = Array.isArray(body.messages) ? body.messages : null;
  if (!messages) {
    return { body, downgradedImages: 0, estimatedBytes };
  }

  let downgradedImages = 0;

  // formatMessages 产出的数组本身就是时间顺序：从最旧的开始丢图，保留最近上下文
  for (const message of messages) {
    if (estimatedBytes <= budgetBytes) break;
    if (!isRecord(message) || !Array.isArray(message.content)) continue;

    const parts = message.content;
    for (let index = 0; index < parts.length; index += 1) {
      if (estimatedBytes <= budgetBytes) break;

      const part = parts[index];
      if (!isRecord(part) || part.type !== 'image_url') continue;
      if (!isRecord(part.image_url)) continue;
      if (typeof part.image_url.url !== 'string' || !part.image_url.url) continue;

      const replacement = { type: 'text', text: IMAGE_PLACEHOLDER };
      // 只替换数组中的一个元素，逗号/括号不变，长度差就是精确的增量
      estimatedBytes += jsonByteLength(replacement) - jsonByteLength(part);
      parts[index] = replacement;
      downgradedImages += 1;
    }
  }

  return { body, downgradedImages, estimatedBytes };
}
