//! 内嵌静态资源。
//!
//! 头像仍放在仓库的 `public/images/`（网页版与桌面版共用一份），
//! 通过 `include_bytes!` 编进二进制，安装后不再需要附带资源目录。

/// 网页版头像路径到字节的映射；未知路径返回 `None`。
pub fn avatar_bytes(path: &str) -> Option<&'static [u8]> {
    Some(match path {
        "./images/char_01.jpg" => include_bytes!("../../../public/images/char_01.jpg").as_slice(),
        "./images/char_02.jpg" => include_bytes!("../../../public/images/char_02.jpg").as_slice(),
        "./images/char_03.jpg" => include_bytes!("../../../public/images/char_03.jpg").as_slice(),
        "./images/user_01.jpg" => include_bytes!("../../../public/images/user_01.jpg").as_slice(),
        "./images/user_02.jpg" => include_bytes!("../../../public/images/user_02.jpg").as_slice(),
        _ => return None,
    })
}

/// 该路径是不是内嵌头像（用于区分「内置头像」与「用户上传的 data URL」）。
pub fn is_builtin_avatar(path: &str) -> bool {
    avatar_bytes(path).is_some()
}

/// 窗口图标：用 Decem 的第一张头像。
pub fn window_icon() -> Option<egui::IconData> {
    let image = crate::media::decode_image(avatar_bytes("./images/char_01.jpg")?)?.into_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}
