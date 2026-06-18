use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessIcon {
    pub png_path: PathBuf,
}

#[cfg(target_os = "windows")]
pub fn cached_icon_for_process_path(process_path: &str) -> Option<ProcessIcon> {
    windows::cached_icon_for_process_path(process_path)
}

#[cfg(not(target_os = "windows"))]
pub fn cached_icon_for_process_path(_process_path: &str) -> Option<ProcessIcon> {
    // 非 Windows 平台的应用图标来源分散在 desktop entry、bundle 或主题缓存中；
    // 当前先保持 UI 回退图标，避免用不可移植路径假装已经拿到真实图标。
    None
}

#[cfg(target_os = "windows")]
mod windows {
    use std::collections::hash_map::DefaultHasher;
    use std::fs;
    use std::hash::{Hash, Hasher};
    use std::path::{Path, PathBuf};

    use image::codecs::png::PngEncoder;
    use image::{ExtendedColorType, ImageEncoder};
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDC, GetDIBits, HGDIOBJ, ReleaseDC, SelectObject,
    };
    use windows_sys::Win32::UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DI_NORMAL, DestroyIcon, DrawIconEx, HICON, PrivateExtractIconsW,
    };

    use super::ProcessIcon;

    const ICON_SIZE: i32 = 64;

    pub(super) fn cached_icon_for_process_path(process_path: &str) -> Option<ProcessIcon> {
        let source = Path::new(process_path);
        if !source.is_file() {
            return None;
        }
        let cache_path = cache_path_for(source);
        if cache_path.is_file() || write_icon_png(source, &cache_path).is_ok() {
            Some(ProcessIcon {
                png_path: cache_path,
            })
        } else {
            None
        }
    }

    fn cache_path_for(source: &Path) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        source
            .to_string_lossy()
            .to_ascii_lowercase()
            .hash(&mut hasher);
        let file_name = format!("{:016x}-{ICON_SIZE}.png", hasher.finish());
        std::env::temp_dir()
            .join("air")
            .join("process-icons")
            .join(file_name)
    }

    fn write_icon_png(source: &Path, cache_path: &Path) -> Result<(), String> {
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let icon = extract_file_icon(source)?;
        let pixels = icon_to_rgba(icon, ICON_SIZE, ICON_SIZE);
        unsafe {
            // SHGetFileInfoW 返回的 HICON 由调用方负责销毁；这里在像素复制完成后立即释放。
            DestroyIcon(icon);
        }
        let pixels = pixels?;
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(
                &pixels,
                ICON_SIZE as u32,
                ICON_SIZE as u32,
                ExtendedColorType::Rgba8,
            )
            .map_err(|error| error.to_string())?;
        fs::write(cache_path, png).map_err(|error| error.to_string())
    }

    fn extract_file_icon(source: &Path) -> Result<HICON, String> {
        let mut wide = source
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let mut icon = std::ptr::null_mut();
        let extracted = unsafe {
            PrivateExtractIconsW(
                wide.as_ptr(),
                0,
                ICON_SIZE,
                ICON_SIZE,
                &mut icon,
                std::ptr::null_mut(),
                1,
                0,
            )
        };
        if extracted > 0 && !icon.is_null() {
            // Windows 可执行文件通常内置多尺寸图标；优先按显示尺寸以上抽取，避免 32px 被 UI 放大后发虚。
            return Ok(icon);
        }

        let mut info = SHFILEINFOW::default();
        let result = unsafe {
            SHGetFileInfoW(
                wide.as_mut_ptr(),
                0,
                &mut info,
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            )
        };
        if result == 0 || info.hIcon.is_null() {
            Err("failed to extract process icon".into())
        } else {
            Ok(info.hIcon)
        }
    }

    fn icon_to_rgba(icon: HICON, width: i32, height: i32) -> Result<Vec<u8>, String> {
        let null_hwnd = std::ptr::null_mut();
        let screen_dc = unsafe { GetDC(null_hwnd) };
        if screen_dc.is_null() {
            return Err("failed to get screen dc".into());
        }
        let memory_dc = unsafe { CreateCompatibleDC(screen_dc) };
        if memory_dc.is_null() {
            unsafe {
                ReleaseDC(null_hwnd, screen_dc);
            }
            return Err("failed to create memory dc".into());
        }
        let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, width, height) };
        if bitmap.is_null() {
            unsafe {
                DeleteDC(memory_dc);
                ReleaseDC(null_hwnd, screen_dc);
            }
            return Err("failed to create icon bitmap".into());
        }

        let old = unsafe { SelectObject(memory_dc, bitmap as HGDIOBJ) };
        let drawn = unsafe {
            DrawIconEx(
                memory_dc,
                0,
                0,
                icon,
                width,
                height,
                0,
                std::ptr::null_mut(),
                DI_NORMAL,
            )
        };
        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize =
            std::mem::size_of::<windows_sys::Win32::Graphics::Gdi::BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bgra = vec![0_u8; (width * height * 4) as usize];
        let copied = if drawn != 0 {
            unsafe {
                GetDIBits(
                    memory_dc,
                    bitmap,
                    0,
                    height as u32,
                    bgra.as_mut_ptr().cast(),
                    &mut info,
                    DIB_RGB_COLORS,
                )
            }
        } else {
            0
        };

        unsafe {
            SelectObject(memory_dc, old);
            DeleteObject(bitmap as HGDIOBJ);
            DeleteDC(memory_dc);
            ReleaseDC(null_hwnd, screen_dc);
        }

        if copied == 0 {
            return Err("failed to copy icon pixels".into());
        }

        for pixel in bgra.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        Ok(bgra)
    }
}
