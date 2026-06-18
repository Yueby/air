use std::borrow::Cow;

use anyhow::anyhow;
use gpui::{AssetSource, Hsla, IntoElement, Pixels, SharedString, Styled, px};
use gpui_component::{Icon as ComponentIcon, IconNamed};
use rust_embed::RustEmbed;

const APP_ICON_PREFIX: &str = "air-icons/";
const APP_BRAND_PREFIX: &str = "air-brand/";
const APP_EMOJI_PREFIX: &str = "emoji/";

/// 应用自身只暴露当前界面确实用到的图标，避免把 `icons/` 下的全量 lucide SVG 都注册进 GPUI 资源表。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Icon {
    Activity,
    AirVent,
    AlertCircle,
    AlertTriangle,
    AppWindow,
    ArrowDown,
    ArrowDownWideNarrow,
    ArrowUp,
    ArrowUpNarrowWide,
    BadgeInfo,
    BellOff,
    Cable,
    Check,
    CheckCircle,
    ChevronDown,
    ChevronRight,
    Circle,
    CircleDashed,
    CircleGauge,
    CircleOff,
    CircleStop,
    CircleX,
    Copy,
    Download,
    FileCog,
    FilePenLine,
    FileSliders,
    FolderOpen,
    Gauge,
    Globe,
    Info,
    Languages,
    Layers,
    ListChecks,
    ListFilter,
    ListTree,
    Loader2,
    MemoryStick,
    MonitorCog,
    Moon,
    Network,
    PanelTopOpen,
    Pencil,
    Play,
    Power,
    Radio,
    RefreshCw,
    RotateCw,
    Save,
    ScrollText,
    SearchX,
    Settings2,
    Sun,
    Terminal,
    Trash2,
    Undo2,
    Upload,
    X,
}

impl Icon {
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Activity => "activity.svg",
            Self::AirVent => "air-vent.svg",
            Self::AlertCircle => "circle-alert.svg",
            Self::AlertTriangle => "triangle-alert.svg",
            Self::AppWindow => "app-window.svg",
            Self::ArrowDown => "arrow-down.svg",
            Self::ArrowDownWideNarrow => "arrow-down-wide-narrow.svg",
            Self::ArrowUp => "arrow-up.svg",
            Self::ArrowUpNarrowWide => "arrow-up-narrow-wide.svg",
            Self::BadgeInfo => "badge-info.svg",
            Self::BellOff => "bell-off.svg",
            Self::Cable => "cable.svg",
            Self::Check => "check.svg",
            Self::CheckCircle => "circle-check.svg",
            Self::ChevronDown => "chevron-down.svg",
            Self::ChevronRight => "chevron-right.svg",
            Self::Circle => "circle.svg",
            Self::CircleDashed => "circle-dashed.svg",
            Self::CircleGauge => "circle-gauge.svg",
            Self::CircleOff => "circle-off.svg",
            Self::CircleStop => "circle-stop.svg",
            Self::CircleX => "circle-x.svg",
            Self::Copy => "copy.svg",
            Self::Download => "download.svg",
            Self::FileCog => "file-cog.svg",
            Self::FilePenLine => "file-pen-line.svg",
            Self::FileSliders => "file-sliders.svg",
            Self::FolderOpen => "folder-open.svg",
            Self::Gauge => "gauge.svg",
            Self::Globe => "globe.svg",
            Self::Info => "info.svg",
            Self::Languages => "languages.svg",
            Self::Layers => "layers.svg",
            Self::ListChecks => "list-checks.svg",
            Self::ListFilter => "list-filter.svg",
            Self::ListTree => "list-tree.svg",
            Self::Loader2 => "loader-circle.svg",
            Self::MemoryStick => "memory-stick.svg",
            Self::MonitorCog => "monitor-cog.svg",
            Self::Moon => "moon.svg",
            Self::Network => "network.svg",
            Self::PanelTopOpen => "panel-top-open.svg",
            Self::Pencil => "pencil.svg",
            Self::Play => "play.svg",
            Self::Power => "power.svg",
            Self::Radio => "radio.svg",
            Self::RefreshCw => "refresh-cw.svg",
            Self::RotateCw => "rotate-cw.svg",
            Self::Save => "save.svg",
            Self::ScrollText => "scroll-text.svg",
            Self::SearchX => "search-x.svg",
            Self::Settings2 => "settings-2.svg",
            Self::Sun => "sun.svg",
            Self::Terminal => "terminal.svg",
            Self::Trash2 => "trash-2.svg",
            Self::Undo2 => "undo-2.svg",
            Self::Upload => "upload.svg",
            Self::X => "x.svg",
        }
    }

    pub fn asset_path(self) -> SharedString {
        format!("{APP_ICON_PREFIX}{}", self.file_name()).into()
    }
}

impl IconNamed for Icon {
    fn path(self) -> SharedString {
        self.asset_path()
    }
}

pub fn brand_banner_asset_path(dark: bool) -> SharedString {
    let file_name = if dark {
        "banner-dark.png"
    } else {
        "banner-light.png"
    };
    format!("{APP_BRAND_PREFIX}{file_name}").into()
}

pub fn brand_icon_asset_path() -> SharedString {
    format!("{APP_BRAND_PREFIX}icon.png").into()
}

pub fn brand_titlebar_icon_asset_path() -> SharedString {
    format!("{APP_BRAND_PREFIX}icon-no-title.png").into()
}

pub fn brand_icon_png_bytes() -> &'static [u8] {
    include_bytes!("../assets/brand/icon.png")
}

pub fn icon(icon: Icon, color: Hsla) -> impl IntoElement {
    ComponentIcon::new(icon).size(px(18.0)).text_color(color)
}

/// 小尺寸图标用于标签、计数器等紧凑控件，避免复用默认 18px 图标挤高行高。
pub fn sized_icon(icon: Icon, color: Hsla, size: Pixels) -> impl IntoElement {
    ComponentIcon::new(icon).size(size).text_color(color)
}

/// RustEmbed 的 include 列表即项目图标白名单；新增图标时必须同时加入 `Icon` 映射和这里的资源声明。
#[derive(RustEmbed)]
#[folder = "assets/icons"]
#[include = "activity.svg"]
#[include = "air-vent.svg"]
#[include = "app-window.svg"]
#[include = "arrow-down.svg"]
#[include = "arrow-down-wide-narrow.svg"]
#[include = "arrow-left.svg"]
#[include = "arrow-right.svg"]
#[include = "arrow-up.svg"]
#[include = "arrow-up-narrow-wide.svg"]
#[include = "asterisk.svg"]
#[include = "badge-info.svg"]
#[include = "bell-off.svg"]
#[include = "cable.svg"]
#[include = "calendar.svg"]
#[include = "case-sensitive.svg"]
#[include = "check.svg"]
#[include = "chevron-down.svg"]
#[include = "chevron-left.svg"]
#[include = "chevron-right.svg"]
#[include = "chevron-up.svg"]
#[include = "chevrons-up-down.svg"]
#[include = "circle.svg"]
#[include = "circle-alert.svg"]
#[include = "circle-check.svg"]
#[include = "circle-dashed.svg"]
#[include = "circle-gauge.svg"]
#[include = "circle-off.svg"]
#[include = "circle-stop.svg"]
#[include = "circle-x.svg"]
#[include = "copy.svg"]
#[include = "cpu.svg"]
#[include = "download.svg"]
#[include = "ellipsis.svg"]
#[include = "external-link.svg"]
#[include = "eye.svg"]
#[include = "eye-off.svg"]
#[include = "file-cog.svg"]
#[include = "file-pen-line.svg"]
#[include = "file-sliders.svg"]
#[include = "folder-open.svg"]
#[include = "gauge.svg"]
#[include = "globe.svg"]
#[include = "inbox.svg"]
#[include = "info.svg"]
#[include = "languages.svg"]
#[include = "layers.svg"]
#[include = "list-checks.svg"]
#[include = "list-filter.svg"]
#[include = "list-tree.svg"]
#[include = "loader.svg"]
#[include = "loader-circle.svg"]
#[include = "maximize.svg"]
#[include = "memory-stick.svg"]
#[include = "minimize.svg"]
#[include = "minus.svg"]
#[include = "monitor-cog.svg"]
#[include = "moon.svg"]
#[include = "network.svg"]
#[include = "panel-bottom.svg"]
#[include = "panel-bottom-open.svg"]
#[include = "panel-left.svg"]
#[include = "panel-left-close.svg"]
#[include = "panel-left-open.svg"]
#[include = "panel-right.svg"]
#[include = "panel-right-close.svg"]
#[include = "panel-right-open.svg"]
#[include = "panel-top-open.svg"]
#[include = "pencil.svg"]
#[include = "play.svg"]
#[include = "plus.svg"]
#[include = "power.svg"]
#[include = "radio.svg"]
#[include = "refresh-cw.svg"]
#[include = "replace.svg"]
#[include = "rotate-cw.svg"]
#[include = "save.svg"]
#[include = "scroll-text.svg"]
#[include = "search.svg"]
#[include = "search-x.svg"]
#[include = "settings-2.svg"]
#[include = "star.svg"]
#[include = "sun.svg"]
#[include = "terminal.svg"]
#[include = "trash-2.svg"]
#[include = "triangle-alert.svg"]
#[include = "undo-2.svg"]
#[include = "upload.svg"]
#[include = "user.svg"]
#[include = "window-restore.svg"]
#[include = "x.svg"]
struct EmbeddedAppIcons;

/// 品牌 banner 是侧栏顶部的完整位图标识，和普通 SVG 图标分开注册，避免图标白名单承担大图资源职责。
#[derive(RustEmbed)]
#[folder = "assets/brand"]
#[include = "banner-dark.png"]
#[include = "banner-light.png"]
#[include = "icon.png"]
#[include = "icon-no-title.png"]
struct EmbeddedBrandImages;

/// emoji 使用 Twemoji SVG 资产渲染，规避 Windows 系统字体和 GPUI 彩色字体支持差异。
#[derive(RustEmbed)]
#[folder = "assets/emoji"]
#[include = "*.svg"]
struct EmbeddedEmojiImages;

pub(crate) fn emoji_image_asset_path(codepoints: &[u32]) -> Option<String> {
    emoji_image_asset_path_from_candidates(codepoints)
        .map(|file_name| format!("{APP_EMOJI_PREFIX}{file_name}"))
}

fn emoji_image_asset_path_from_candidates(codepoints: &[u32]) -> Option<String> {
    for candidate in emoji_asset_candidates(codepoints) {
        if EmbeddedEmojiImages::get(&candidate).is_some() {
            return Some(candidate);
        }
    }
    None
}

fn emoji_asset_candidates(codepoints: &[u32]) -> Vec<String> {
    let mut candidates = Vec::new();
    if codepoints.is_empty() {
        return candidates;
    }

    candidates.push(emoji_asset_file_name(codepoints));
    let without_variation_selectors = codepoints
        .iter()
        .copied()
        .filter(|codepoint| !matches!(*codepoint, 0xfe0e | 0xfe0f))
        .collect::<Vec<_>>();
    if without_variation_selectors.len() != codepoints.len() {
        candidates.push(emoji_asset_file_name(&without_variation_selectors));
    }

    candidates
}

fn emoji_asset_file_name(codepoints: &[u32]) -> String {
    let stem = codepoints
        .iter()
        .map(|codepoint| format!("{codepoint:x}"))
        .collect::<Vec<_>>()
        .join("-");
    format!("{stem}.svg")
}

/// 组合项目 SVG 和 gpui-component 内置 SVG 路径：项目图标走 `air-icons/`，组件库图标继续走 `icons/`。
pub struct AppAssets;

impl AppAssets {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AppAssets {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if let Some(icon_path) = path.strip_prefix(APP_ICON_PREFIX) {
            return EmbeddedAppIcons::get(icon_path)
                .map(|file| Some(file.data))
                .ok_or_else(|| anyhow!("could not find app icon at path \"{path}\""));
        }

        if let Some(image_path) = path.strip_prefix(APP_BRAND_PREFIX) {
            return EmbeddedBrandImages::get(image_path)
                .map(|file| Some(file.data))
                .ok_or_else(|| anyhow!("could not find brand image at path \"{path}\""));
        }

        if let Some(emoji_path) = path.strip_prefix(APP_EMOJI_PREFIX) {
            return EmbeddedEmojiImages::get(emoji_path)
                .map(|file| Some(file.data))
                .ok_or_else(|| anyhow!("could not find emoji image at path \"{path}\""));
        }

        if let Some(icon_path) = path.strip_prefix("icons/") {
            let icon_path = component_icon_alias(icon_path);
            return EmbeddedAppIcons::get(icon_path)
                .map(|file| Some(file.data))
                .ok_or_else(|| anyhow!("could not find component icon at path \"{path}\""));
        }

        Ok(None)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut assets = Vec::new();

        if APP_ICON_PREFIX.starts_with(path) || path.starts_with(APP_ICON_PREFIX) {
            let embedded_path = path.strip_prefix(APP_ICON_PREFIX).unwrap_or("");
            assets.extend(
                EmbeddedAppIcons::iter()
                    .filter(|icon| icon.starts_with(embedded_path))
                    .map(|icon| format!("{APP_ICON_PREFIX}{icon}").into()),
            );
        }

        if APP_BRAND_PREFIX.starts_with(path) || path.starts_with(APP_BRAND_PREFIX) {
            let embedded_path = path.strip_prefix(APP_BRAND_PREFIX).unwrap_or("");
            assets.extend(
                EmbeddedBrandImages::iter()
                    .filter(|image| image.starts_with(embedded_path))
                    .map(|image| format!("{APP_BRAND_PREFIX}{image}").into()),
            );
        }

        if APP_EMOJI_PREFIX.starts_with(path) || path.starts_with(APP_EMOJI_PREFIX) {
            let embedded_path = path.strip_prefix(APP_EMOJI_PREFIX).unwrap_or("");
            assets.extend(
                EmbeddedEmojiImages::iter()
                    .filter(|emoji| emoji.starts_with(embedded_path))
                    .map(|emoji| format!("{APP_EMOJI_PREFIX}{emoji}").into()),
            );
        }

        if "icons/".starts_with(path) || path.starts_with("icons/") {
            let embedded_path = path.strip_prefix("icons/").unwrap_or("");
            assets.extend(
                EmbeddedAppIcons::iter()
                    .filter(|icon| icon.starts_with(embedded_path))
                    .map(|icon| format!("icons/{icon}").into()),
            );
        }

        Ok(assets)
    }
}

fn component_icon_alias(icon_path: &str) -> &str {
    match icon_path {
        // gpui-component 有少量非 lucide 文件名；这里用本项目 SVG 库中的近似图标兜底，保持内置 IconName 可解析。
        "close.svg" | "window-close.svg" => "x.svg",
        "inspector.svg" => "search.svg",
        "resize-corner.svg" => "panel-bottom-open.svg",
        "sort-ascending.svg" => "arrow-up-narrow-wide.svg",
        "sort-descending.svg" => "arrow-down-wide-narrow.svg",
        "star-fill.svg" => "star.svg",
        "window-maximize.svg" => "maximize.svg",
        "window-minimize.svg" => "minimize.svg",
        "window-restore.svg" => "window-restore.svg",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        APP_EMOJI_PREFIX, APP_ICON_PREFIX, AppAssets, Icon, brand_banner_asset_path,
        brand_icon_asset_path, emoji_image_asset_path,
    };
    use gpui::AssetSource;

    #[test]
    fn app_icon_paths_are_registered_under_project_prefix() {
        assert_eq!(
            Icon::AirVent.asset_path().as_ref(),
            "air-icons/air-vent.svg"
        );
        assert!(Icon::AlertCircle.asset_path().starts_with(APP_ICON_PREFIX));
    }

    #[test]
    fn app_assets_load_project_and_component_icons() {
        let assets = AppAssets::new();

        assert!(
            assets
                .load(Icon::AirVent.asset_path().as_ref())
                .expect("project icon load should not fail")
                .is_some()
        );
        assert!(
            assets
                .load("icons/settings-2.svg")
                .expect("component icon load should not fail")
                .is_some()
        );
        assert!(
            assets
                .load("icons/close.svg")
                .expect("component alias icon load should not fail")
                .is_some()
        );
        assert!(
            assets
                .load(brand_banner_asset_path(false).as_ref())
                .expect("light brand banner should not fail")
                .is_some()
        );
        assert!(
            assets
                .load(brand_banner_asset_path(true).as_ref())
                .expect("dark brand banner should not fail")
                .is_some()
        );
        assert!(
            assets
                .load(brand_icon_asset_path().as_ref())
                .expect("brand icon load should not fail")
                .is_some()
        );
    }

    #[test]
    fn app_assets_embed_emoji_images() {
        let assets = AppAssets::new();
        let flag_path = format!("{APP_EMOJI_PREFIX}1f1ed-1f1f0.svg");
        let flag = assets
            .load(&flag_path)
            .expect("embedded emoji flag load should not fail")
            .expect("emoji flag should be embedded");
        assert!(flag.len() > 1024);
        assert!(
            assets
                .list(APP_EMOJI_PREFIX)
                .expect("embedded emoji list should load")
                .iter()
                .any(|path| path.as_ref() == flag_path)
        );
    }

    #[test]
    fn emoji_asset_path_supports_variation_selector_aliases() {
        assert_eq!(
            emoji_image_asset_path(&[0x2764, 0xfe0f]).as_deref(),
            Some("emoji/2764.svg")
        );
        assert_eq!(
            emoji_image_asset_path(&[0x31, 0xfe0f, 0x20e3]).as_deref(),
            Some("emoji/31-20e3.svg")
        );
    }
}
