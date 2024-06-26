use image::RgbaImage;
use std::mem;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, GetDIBits, SelectObject, BITMAPINFO,
        BITMAPINFOHEADER, DIB_RGB_COLORS, SRCCOPY,
    },
    UI::WindowsAndMessaging::{
        GetDesktopWindow, GetSystemMetrics, SetProcessDPIAware, SM_CXSCREEN, SM_CYSCREEN,
    },
};

use crate::{
    error::{XCapError, XCapResult},
    platform::utils::get_window_rect,
};

use super::{
    boxed::{BoxHBITMAP, BoxHDC},
    utils::get_os_major_version,
};

fn to_rgba_image(
    box_hdc_mem: BoxHDC,
    box_h_bitmap: BoxHBITMAP,
    width: i32,
    height: i32,
) -> XCapResult<RgbaImage> {
    let buffer_size = width * height * 4;
    let mut bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biSizeImage: buffer_size as u32,
            biCompression: 0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut buffer = vec![0u8; buffer_size as usize];

    unsafe {
        // 读取数据到 buffer 中
        let is_success = GetDIBits(
            *box_hdc_mem,
            *box_h_bitmap,
            0,
            height as u32,
            Some(buffer.as_mut_ptr().cast()),
            &mut bitmap_info,
            DIB_RGB_COLORS,
        ) == 0;

        if is_success {
            return Err(XCapError::new("Get RGBA data failed"));
        }
    };

    for src in buffer.chunks_exact_mut(4) {
        src.swap(0, 2);
        // fix https://github.com/nashaofu/xcap/issues/92#issuecomment-1910014951
        if src[3] == 0 && get_os_major_version() < 8 {
            src[3] = 255;
        }
    }

    RgbaImage::from_raw(width as u32, height as u32, buffer)
        .ok_or_else(|| XCapError::new("RgbaImage::from_raw failed"))
}

#[allow(unused)]
pub fn capture_monitor(x: i32, y: i32, width: i32, height: i32) -> XCapResult<RgbaImage> {
    unsafe {
        SetProcessDPIAware();
        let hwnd = GetDesktopWindow();
        let box_hdc_desktop_window = BoxHDC::from(hwnd);

        // 内存中的HDC，使用 DeleteDC 函数释放
        // https://learn.microsoft.com/zh-cn/windows/win32/api/wingdi/nf-wingdi-createcompatibledc
        let box_hdc_mem = BoxHDC::new(CreateCompatibleDC(*box_hdc_desktop_window), None);
        let box_h_bitmap = BoxHBITMAP::new(CreateCompatibleBitmap(
            *box_hdc_desktop_window,
            width,
            height,
        ));

        // 使用SelectObject函数将这个位图选择到DC中
        SelectObject(*box_hdc_mem, *box_h_bitmap);

        // 拷贝原始图像到内存
        // 这里不需要缩放图片，所以直接使用BitBlt
        // 如需要缩放，则使用 StretchBlt
        BitBlt(
            *box_hdc_mem,
            0,
            0,
            width,
            height,
            *box_hdc_desktop_window,
            x,
            y,
            SRCCOPY,
        )?;

        to_rgba_image(box_hdc_mem, box_h_bitmap, width, height)
    }
}

#[allow(unused)]
pub fn capture_window(hwnd: HWND, scale_factor: f32) -> XCapResult<RgbaImage> {
    unsafe {
        SetProcessDPIAware();
        let dw_hwnd = GetDesktopWindow();
        let box_hdc_desktop_window: BoxHDC = BoxHDC::from(dw_hwnd);
        let box_hdc_window: BoxHDC = BoxHDC::from(hwnd);
        let rect = get_window_rect(hwnd)?;
        let mut width = rect.right - rect.left;
        let mut height = rect.bottom - rect.top;

        if width == 0 {
            width = GetSystemMetrics(SM_CXSCREEN);
        }
        if height == 0 {
            height = GetSystemMetrics(SM_CYSCREEN);
        }

        let mut horizontal_scale = 1.0;
        let mut vertical_scale = 1.0;

        width = (width as f32 * scale_factor) as i32;
        height = (height as f32 * scale_factor) as i32;

        // 内存中的HDC，使用 DeleteDC 函数释放
        // https://learn.microsoft.com/zh-cn/windows/win32/api/wingdi/nf-wingdi-createcompatibledc
        let box_hdc_mem = BoxHDC::new(CreateCompatibleDC(*box_hdc_desktop_window), None);
        let box_h_bitmap = BoxHBITMAP::new(CreateCompatibleBitmap(
            *box_hdc_desktop_window,
            width,
            height,
        ));

        let previous_object = SelectObject(*box_hdc_mem, *box_h_bitmap);

        let mut is_success = false;

        if !is_success {
            is_success = BitBlt(
                *box_hdc_mem,
                0,
                0,
                width,
                height,
                *box_hdc_desktop_window,
                rect.left,
                rect.top,
                SRCCOPY,
            )
            .is_ok();
        }

        SelectObject(*box_hdc_mem, previous_object);

        to_rgba_image(box_hdc_mem, box_h_bitmap, width, height)
    }
}
