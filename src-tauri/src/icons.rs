// Real application icons.
//
// For an installed app we extract the actual icon embedded in its executable
// and hand it to the UI as a PNG data URL. This is the real app logo, not a
// placeholder. If extraction fails for any reason the command returns None and
// the UI falls back to a clean category icon.

use base64::Engine;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::UI::Shell::SHDefExtractIconW;
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// Encode a top-down RGBA buffer to PNG bytes.
fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

// Pull the pixels out of an icon's color bitmap and return a PNG data URL.
unsafe fn hicon_to_data_url(hicon: HICON) -> Option<String> {
    unsafe {
    let mut info = ICONINFO::default();
    if GetIconInfo(hicon, &mut info).is_err() {
        return None;
    }
    let hbm_color = info.hbmColor;
    let hbm_mask = info.hbmMask;

    // Measure the color bitmap.
    let mut bmp = BITMAP::default();
    let got = GetObjectW(
        HGDIOBJ(hbm_color.0),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bmp as *mut _ as *mut c_void),
    );
    if got == 0 || bmp.bmWidth <= 0 || bmp.bmHeight <= 0 {
        let _ = DeleteObject(HGDIOBJ(hbm_color.0));
        let _ = DeleteObject(HGDIOBJ(hbm_mask.0));
        return None;
    }
    let width = bmp.bmWidth as u32;
    let height = bmp.bmHeight as u32;

    // Ask GDI for 32-bit top-down pixels (negative height).
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: bmp.bmWidth,
            biHeight: -bmp.bmHeight,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut buffer = vec![0u8; (width * height * 4) as usize];
    let hdc = GetDC(HWND(std::ptr::null_mut()));
    let scanlines = GetDIBits(
        hdc,
        hbm_color,
        0,
        height,
        Some(buffer.as_mut_ptr() as *mut c_void),
        &mut bmi,
        DIB_RGB_COLORS,
    );
    ReleaseDC(HWND(std::ptr::null_mut()), hdc);
    let _ = DeleteObject(HGDIOBJ(hbm_color.0));
    let _ = DeleteObject(HGDIOBJ(hbm_mask.0));

    if scanlines == 0 {
        return None;
    }

    // GDI gives BGRA. Swap to RGBA. If every alpha byte is zero (some older
    // icons), treat the image as fully opaque so it is not invisible.
    let mut any_alpha = false;
    for px in buffer.chunks_exact(4) {
        if px[3] != 0 {
            any_alpha = true;
            break;
        }
    }
    for px in buffer.chunks_exact_mut(4) {
        px.swap(0, 2); // B <-> R
        if !any_alpha {
            px[3] = 255;
        }
    }

    let png = encode_png(width, height, &buffer)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    Some(format!("data:image/png;base64,{}", b64))
    }
}

// Extract the icon for one executable path.
pub fn extract_icon(path: &str) -> Option<String> {
    if !Path::new(path).exists() {
        return None;
    }
    let wide = to_wide(path);
    let mut large = HICON::default();
    // Request a 64px large icon. LOWORD = large size, HIWORD = small size.
    let size: u32 = 64 | (16 << 16);
    let hr = unsafe {
        SHDefExtractIconW(
            PCWSTR(wide.as_ptr()),
            0,
            0,
            Some(&mut large),
            None,
            size,
        )
    };
    if hr.is_err() || large.is_invalid() {
        return None;
    }
    let result = unsafe { hicon_to_data_url(large) };
    unsafe {
        let _ = DestroyIcon(large);
    }
    result
}

// Extract icons for many paths at once. Keyed by the path passed in.
pub fn extract_icons(paths: Vec<String>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for p in paths {
        if let Some(url) = extract_icon(&p) {
            map.insert(p, url);
        }
    }
    map
}
