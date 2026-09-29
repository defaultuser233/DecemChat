//! 构建脚本：准备前端产物、生成内嵌资源，再交给 tauri-build。

use std::env;
use std::fs;
use std::path::PathBuf;

/// 前端没构建时的占位页——保证 `cargo build` 不被卡住（正式构建先跑 `pnpm build`）。
const PLACEHOLDER: &str = r#"<!doctype html>
<html lang="zh-CN"><head><meta charset="UTF-8"><title>Decem Chat</title></head>
<body style="font-family: system-ui; padding: 2rem">
<h1>前端还没有构建</h1>
<p>先跑 <code>pnpm build</code> 生成 <code>dist/</code>，再重新编译本外壳。</p>
</body></html>
"#;

fn main() {
    let manifest =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("构建时应有 CARGO_MANIFEST_DIR"));
    let dist = manifest
        .parent()
        .expect("src-tauri 一定在仓库根目录下")
        .join("dist");

    if !dist.join("index.html").is_file() {
        fs::create_dir_all(&dist).expect("无法创建 dist 目录");
        fs::write(dist.join("index.html"), PLACEHOLDER).expect("无法写入占位 index.html");
        println!("cargo:warning=dist/ 里没有 index.html，已写入占位页；正式构建请先跑 pnpm build");
    }
    println!("cargo:rerun-if-changed={}", dist.display());

    // 把前端产物所在目录写成一个 Rust 字面量，供 `include_dir!` 内嵌（宏只接受字面量）
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("构建时应有 OUT_DIR"));
    fs::write(
        out_dir.join("dist.rs"),
        format!(
            "/// 前端构建产物（编译期内嵌，见 build.rs）。\npub static DIST: include_dir::Dir<'static> = include_dir::include_dir!({path:?});\n",
            path = dist.display().to_string()
        ),
    )
    .expect("无法生成 dist.rs");

    tauri_build::build();

    // tauri-build 会在自己的 out 目录里放一个**故意无效**的 msvcrt.lib（配合它发出的
    // `/NODEFAULTLIB:msvcrt.lib`）。这个小文件会污染 `cargo test --workspace` 时传给
    // 其它 crate doctest 的链接搜索路径，把真正的系统 msvcrt.lib 挡掉——实测表现为
    // LNK4003（无效库格式，已忽略）+ LNK1120，decem-core 的 doctest 链接失败。
    // 它本身不提供任何符号，删掉不影响外壳自己的链接。
    let shadow = out_dir.join("msvcrt.lib");
    if shadow.is_file()
        && let Err(err) = fs::remove_file(&shadow)
    {
        println!("cargo:warning=无法移除 tauri-build 的占位 msvcrt.lib：{err}");
    }
}
