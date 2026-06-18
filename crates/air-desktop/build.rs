use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rerun-if-changed={}",
        manifest_path("assets/app-icon.png").display()
    );

    if target_is_windows() {
        embed_windows_app_icon();
    }
}

fn target_is_windows() -> bool {
    env::var("TARGET")
        .map(|target| target.contains("windows"))
        .unwrap_or(false)
}

fn embed_windows_app_icon() {
    // Windows 可执行文件图标在构建期从 PNG 转成 ICO 并嵌入资源，保持仓库内只维护单一源素材。
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR should be set by cargo"));
    let icon_png = manifest_path("assets/app-icon.png");
    let icon_ico = out_dir.join("air-app-icon.ico");
    let resource_rc = out_dir.join("air-app-icon.rc");

    let icon = image::open(&icon_png)
        .unwrap_or_else(|error| panic!("failed to load {}: {error}", icon_png.display()));
    // ICO 单张位图最大 256px，这里在构建期统一缩放，避免额外维护独立 ico 文件。
    icon.thumbnail(256, 256)
        .save_with_format(&icon_ico, image::ImageFormat::Ico)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", icon_ico.display()));

    let icon_ico = icon_ico.to_string_lossy().replace('\\', "/");
    let rc_contents = format!("1 ICON \"{icon_ico}\"\n");
    fs::write(&resource_rc, rc_contents)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", resource_rc.display()));

    embed_resource::compile(&resource_rc, embed_resource::NONE)
        .manifest_optional()
        .unwrap_or_else(|error| panic!("failed to compile {}: {error}", resource_rc.display()));
}

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}
