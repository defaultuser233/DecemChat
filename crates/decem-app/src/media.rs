//! 图片加工：data URL 编解码、纹理转换，以及 GIF 抽帧拼接。
//!
//! GIF 部分移植自前端 `src/services/gifFrames.ts`：按时间等间隔取关键帧，
//! 缩放后拼成带序号的网格图。`image` crate 的 GIF 解码器已经按 disposal
//! 规则把每帧还原成完整画面，这里只需要缩放与排版。

use std::io::Cursor;
use std::path::Path;

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::{AnimationDecoder, DynamicImage, Rgba, RgbaImage};

/// 最多抽取的 GIF 帧数。
pub const GIF_MAX_FRAMES: usize = 6;
/// 每帧最长边像素，避免拼接图过大。
pub const GIF_MAX_SIDE: u32 = 512;
/// 拼图间距。
const GAP: u32 = 6;
/// 每格下方留给序号的空白高度。
const LABEL_HEIGHT: u32 = 20;
/// 上传图片体积上限（与网页版一致）。
pub const MAX_UPLOAD_BYTES: usize = 5 * 1024 * 1024;
/// 用户头像缩放后的最长边。
pub const AVATAR_MAX_SIDE: u32 = 256;

/// 按扩展名猜 MIME；未知一律按 `image/png` 处理。
pub fn mime_from_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "image/png",
    }
}

/// 把字节编码成 data URL。
pub fn to_data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", BASE64.encode(bytes))
}

/// 取出 data URL 里的原始字节。
pub fn decode_data_url(data_url: &str) -> Option<Vec<u8>> {
    let (_, payload) = data_url.split_once(',')?;
    if !data_url.starts_with("data:") {
        return None;
    }
    BASE64.decode(payload.trim()).ok()
}

/// 解码图片字节为动态图像。
pub fn decode_image(bytes: &[u8]) -> Option<DynamicImage> {
    image::load_from_memory(bytes)
        .inspect_err(|err| log::warn!("图片解码失败：{err}"))
        .ok()
}

/// 转成 egui 纹理数据。
pub fn to_color_image(image: &DynamicImage) -> egui::ColorImage {
    let rgba = image.to_rgba8();
    egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    )
}

/// 解码字节并转成 egui 纹理数据。
pub fn color_image_from_bytes(bytes: &[u8]) -> Option<egui::ColorImage> {
    decode_image(bytes).map(|image| to_color_image(&image))
}

/// 把图片等比缩放到最长边不超过 `max_side`（只在需要时缩小）。
pub fn scaled(image: DynamicImage, max_side: u32) -> DynamicImage {
    let (width, height) = (image.width(), image.height());
    let longest = width.max(height);
    if longest <= max_side {
        return image;
    }
    let ratio = f64::from(max_side) / f64::from(longest);
    let new_width = ((f64::from(width) * ratio).round() as u32).max(1);
    let new_height = ((f64::from(height) * ratio).round() as u32).max(1);
    image.resize_exact(new_width, new_height, image::imageops::FilterType::Triangle)
}

/// GIF 抽帧结果。
#[derive(Debug, Clone)]
pub struct GifGrid {
    /// 拼接后的 PNG data URL。
    pub data_url: String,
    /// 实际使用的帧数。
    pub frame_count: usize,
}

/// 从 GIF 字节抽取关键帧并拼成网格图；不是 GIF 或解码失败时返回 `None`。
pub fn extract_gif_grid(bytes: &[u8], max_frames: usize) -> Option<GifGrid> {
    let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
        .inspect_err(|err| log::warn!("GIF 解析失败：{err}"))
        .ok()?;
    let frames = decoder
        .into_frames()
        .collect_frames()
        .inspect_err(|err| log::warn!("GIF 抽帧失败：{err}"))
        .ok()?;
    if frames.is_empty() {
        return None;
    }

    let delays: Vec<u32> = frames
        .iter()
        .map(|frame| {
            let (numer, denom) = frame.delay().numer_denom_ms();
            numer.checked_div(denom).unwrap_or(10)
        })
        .collect();
    let indices = pick_frame_indices(&delays, max_frames);
    if indices.is_empty() {
        return None;
    }

    // 只挑出需要的帧，并且直接拿走所有权（`Frame::into_buffer`），避免整幅像素复制
    let selected: Vec<RgbaImage> = frames
        .into_iter()
        .enumerate()
        .filter(|(index, _)| indices.contains(index))
        .map(|(_, frame)| frame.into_buffer())
        .collect();
    if selected.is_empty() {
        return None;
    }

    let grid = combine_to_grid(&selected)?;
    let data_url = encode_png_data_url(&grid).ok()?;
    Some(GifGrid {
        data_url,
        frame_count: selected.len(),
    })
}

/// 按时间等间隔选取至多 `max_frames` 个关键帧（移植自 `pickFrameIndicesByTime`）。
fn pick_frame_indices(delays: &[u32], max_frames: usize) -> Vec<usize> {
    let n = delays.len();
    if n == 0 || max_frames == 0 {
        return Vec::new();
    }
    if n <= max_frames {
        return (0..n).collect();
    }
    if max_frames == 1 {
        return vec![0];
    }

    // delay 缺失或为 0 时按最小单位计，单位不影响相对采样
    let weights: Vec<u64> = delays
        .iter()
        .map(|delay| u64::from(if *delay > 0 { *delay } else { 10 }))
        .collect();
    let total: u64 = weights.iter().sum();

    let mut indices = Vec::with_capacity(max_frames);
    let mut cumulative: u64 = 0;
    let mut frame_index = 0usize;
    for step in 0..max_frames {
        let target = (step as u64 * total) / (max_frames as u64 - 1);
        while frame_index < n - 1 && cumulative + weights[frame_index] <= target {
            cumulative += weights[frame_index];
            frame_index += 1;
        }
        if indices.last() != Some(&frame_index) {
            indices.push(frame_index);
        }
    }
    indices
}

/// 网格布局：1-3 帧单行，4 帧 2×2，5-6 帧 3×2。
fn layout_grid(count: usize) -> (u32, u32) {
    match count {
        0 => (1, 1),
        1..=3 => (count as u32, 1),
        4 => (2, 2),
        _ => (3, count.div_ceil(3) as u32),
    }
}

fn combine_to_grid(frames: &[RgbaImage]) -> Option<RgbaImage> {
    let scaled: Vec<RgbaImage> = frames
        .iter()
        .map(|frame| {
            let dynamic = DynamicImage::ImageRgba8(frame.clone());
            let rgba = scaled(dynamic, GIF_MAX_SIDE).to_rgba8();
            RgbaImage::from_fn(rgba.width(), rgba.height(), |x, y| {
                over_white(*rgba.get_pixel(x, y))
            })
        })
        .collect();

    let cell_w = scaled.iter().map(RgbaImage::width).max()?;
    let cell_h = scaled.iter().map(RgbaImage::height).max()?;
    let (cols, rows) = layout_grid(scaled.len());

    let width = cols * cell_w + (cols - 1) * GAP;
    let height = rows * (cell_h + LABEL_HEIGHT) + (rows - 1) * GAP;
    let mut grid = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));

    for (index, frame) in scaled.iter().enumerate() {
        let col = (index as u32) % cols;
        let row = (index as u32) / cols;
        let x = col * (cell_w + GAP);
        let y = row * (cell_h + LABEL_HEIGHT + GAP);
        image::imageops::overlay(&mut grid, frame, i64::from(x), i64::from(y));
        draw_index(&mut grid, index + 1, x + 2, y + cell_h + 4);
    }

    Some(grid)
}

/// 把带透明度的像素压到白底上（GIF 透明区在网格图里应该是白的）。
fn over_white(pixel: Rgba<u8>) -> Rgba<u8> {
    let alpha = u32::from(pixel[3]);
    let blend =
        |channel: u8| -> u8 { ((u32::from(channel) * alpha + 255 * (255 - alpha)) / 255) as u8 };
    Rgba([blend(pixel[0]), blend(pixel[1]), blend(pixel[2]), 255])
}

/// 3×5 点阵数字：无需字体文件即可画出帧序号。
const DIGIT_ROWS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b010, 0b010, 0b010, 0b010], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b001, 0b001, 0b001], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

/// 在 `(x, y)` 处画出序号（点阵放大 2 倍，约 10px 高）。
fn draw_index(image: &mut RgbaImage, index: usize, x: u32, y: u32) {
    const SCALE: u32 = 2;
    const DOT: Rgba<u8> = Rgba([0x66, 0x66, 0x66, 255]);

    let digits: Vec<u8> = index
        .to_string()
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect();

    let mut cursor_x = x;
    for digit in digits {
        let rows = DIGIT_ROWS[usize::from(digit)];
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..3u32 {
                if bits & (0b100 >> col) == 0 {
                    continue;
                }
                for dy in 0..SCALE {
                    for dx in 0..SCALE {
                        let px = cursor_x + col * SCALE + dx;
                        let py = y + row as u32 * SCALE + dy;
                        if px < image.width() && py < image.height() {
                            image.put_pixel(px, py, DOT);
                        }
                    }
                }
            }
        }
        cursor_x += 3 * SCALE + SCALE;
    }
}

/// PNG 编码并包成 data URL。
pub fn encode_png_data_url(image: &RgbaImage) -> Result<String> {
    let mut buffer = Vec::new();
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut Cursor::new(&mut buffer), image::ImageFormat::Png)
        .context("PNG 编码失败")?;
    Ok(to_data_url("image/png", &buffer))
}

/// 用户头像：等比缩到最长边 256 像素再编成 JPEG data URL，避免配置文件被原图撑爆。
pub fn avatar_data_url(bytes: &[u8]) -> Option<String> {
    let image = scaled(decode_image(bytes)?, AVATAR_MAX_SIDE);
    let rgb = image.to_rgb8();
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 85);
    encoder
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .inspect_err(|err| log::warn!("头像编码失败：{err}"))
        .ok()?;
    Some(to_data_url("image/jpeg", &buffer))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(width, height, Rgba(color))
    }

    #[test]
    fn data_url_round_trip() {
        let url = to_data_url("image/png", &[1, 2, 3, 4]);
        assert!(url.starts_with("data:image/png;base64,"));
        assert_eq!(decode_data_url(&url).as_deref(), Some(&[1u8, 2, 3, 4][..]));
        assert_eq!(decode_data_url("not-a-data-url"), None);
    }

    #[test]
    fn mime_is_guessed_from_extension() {
        assert_eq!(mime_from_path(Path::new("a.JPG")), "image/jpeg");
        assert_eq!(mime_from_path(Path::new("a.gif")), "image/gif");
        assert_eq!(mime_from_path(Path::new("无扩展名")), "image/png");
    }

    #[test]
    fn frames_are_picked_evenly_by_time() {
        // 帧数不超过上限时全部保留
        assert_eq!(pick_frame_indices(&[10, 10, 10], 6), vec![0, 1, 2]);
        // 均匀延时下 8 帧取 6 帧，首尾必取
        let indices = pick_frame_indices(&[10; 8], 6);
        assert_eq!(indices.first(), Some(&0));
        assert_eq!(indices.last(), Some(&7));
        assert!(indices.len() <= 6);
        // 零延时应按最小单位处理而不是除零
        assert!(pick_frame_indices(&[0, 0, 0, 0, 0, 0, 0, 0], 4).len() <= 4);
    }

    #[test]
    fn grid_layout_matches_the_web_version() {
        assert_eq!(layout_grid(1), (1, 1));
        assert_eq!(layout_grid(3), (3, 1));
        assert_eq!(layout_grid(4), (2, 2));
        assert_eq!(layout_grid(6), (3, 2));
    }

    #[test]
    fn grid_is_white_backed_and_labelled() {
        let frames = vec![solid(20, 10, [0, 0, 0, 0]), solid(20, 10, [255, 0, 0, 255])];
        let grid = combine_to_grid(&frames).expect("拼接应成功");
        // 宽 = 2 格 + 间距；高 = 单行（图高 + 序号高度）
        assert_eq!(grid.width(), 2 * 20 + GAP);
        assert_eq!(grid.height(), 10 + LABEL_HEIGHT);
        // 透明像素被压成白色
        assert_eq!(*grid.get_pixel(0, 0), Rgba([255, 255, 255, 255]));
        // 序号落在第二格下方
        let label_y = 10 + 8;
        let painted = (20 + GAP..grid.width())
            .any(|x| *grid.get_pixel(x, label_y) == Rgba([0x66, 0x66, 0x66, 255]));
        assert!(painted, "第二帧应有序号");
    }

    #[test]
    fn scaling_only_shrinks() {
        let small = DynamicImage::ImageRgba8(solid(100, 50, [255, 255, 255, 255]));
        assert_eq!(scaled(small, GIF_MAX_SIDE).width(), 100);
        let big = DynamicImage::ImageRgba8(solid(1024, 512, [255, 255, 255, 255]));
        let resized = scaled(big, GIF_MAX_SIDE);
        assert_eq!(resized.width(), GIF_MAX_SIDE);
        assert_eq!(resized.height(), GIF_MAX_SIDE / 2);
    }

    #[test]
    fn avatar_is_shrunk_to_a_jpeg_data_url() {
        let png = {
            let mut buffer = Vec::new();
            DynamicImage::ImageRgba8(solid(800, 400, [10, 20, 30, 255]))
                .write_to(&mut Cursor::new(&mut buffer), image::ImageFormat::Png)
                .expect("编码失败");
            buffer
        };
        let data_url = avatar_data_url(&png).expect("应能生成头像");
        assert!(data_url.starts_with("data:image/jpeg;base64,"));

        let decoded = decode_data_url(&data_url).expect("应能解回字节");
        let image = decode_image(&decoded).expect("应是合法 JPEG");
        assert_eq!(image.width(), AVATAR_MAX_SIDE);
        assert!(image.height() <= AVATAR_MAX_SIDE);
        // 原图几十 KB 的 PNG 应该明显变小
        assert!(decoded.len() < png.len());
    }
}
