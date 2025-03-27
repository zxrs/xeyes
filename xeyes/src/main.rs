#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::Cell;

use anyhow::Result;
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreatePen, Ellipse, EndPaint, HDC, InvalidateRect, PAINTSTRUCT, PS_SOLID,
            SelectObject, UpdateWindow,
        },
        UI::WindowsAndMessaging::{
            CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
            GetWindowRect, MSG, MSLLHOOKSTRUCT, PostQuitMessage, RegisterClassW, SW_SHOW,
            ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_CREATE, WM_DESTROY, WM_PAINT,
            WM_USER, WNDCLASSW, WS_CAPTION, WS_OVERLAPPED, WS_SYSMENU, WS_VISIBLE,
        },
    },
    core::{Owned, PCWSTR, w},
};

#[link(name = "hook.dll", kind = "static")]
unsafe extern "C" {
    fn set_hook(hwnd: HWND) -> bool;
    fn end_hook() -> bool;
}

const CLASS_NAME: PCWSTR = w!("xeyes_window_class");
const WM_HOOK_MOUSE_POS: u32 = WM_USER + 42;

thread_local! {
    static POS: Cell<Option<POINT>> = const { Cell::new(None) };
}

fn draw_circle(hdc: HDC, top: i32, left: i32, bottom: i32, right: i32) {
    let pen = unsafe { CreatePen(PS_SOLID, 10, COLORREF::default()) };
    let pen = unsafe { Owned::new(pen) };
    let old_pen = unsafe { SelectObject(hdc, (*pen).into()) };

    _ = unsafe { Ellipse(hdc, left, top, right, bottom) };

    unsafe { SelectObject(hdc, old_pen) };
}

fn draw_iris(hdc: HDC, mouse_pos: POINT, center_of_eye: POINT, offset_x: f32) {
    let dx_from_eye = mouse_pos.x - center_of_eye.x;
    let dy_from_eye = mouse_pos.y - center_of_eye.y;

    let distance_from_eye = (dx_from_eye.pow(2) as f32 + dy_from_eye.pow(2) as f32).sqrt();

    if distance_from_eye > 0.0 {
        let dia = if distance_from_eye > 50.0 {
            50.0
        } else {
            distance_from_eye
        };
        let iris_pos = POINT {
            x: (dia * dx_from_eye as f32 / distance_from_eye / 1.76 + offset_x) as _,

            y: (dia * dy_from_eye as f32 / distance_from_eye + 80.0) as _,
        };

        draw_circle(
            hdc,
            iris_pos.y - 18,
            iris_pos.x - 10,
            iris_pos.y + 18,
            iris_pos.x + 10,
        );
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => unsafe {
            set_hook(hwnd);
        },
        WM_DESTROY => unsafe {
            end_hook();
            PostQuitMessage(0)
        },
        WM_HOOK_MOUSE_POS => {
            let ms = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
            POS.set(Some(ms.pt));
            _ = unsafe { InvalidateRect(Some(hwnd), None, true) };
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            // left eye
            draw_circle(hdc, 5, 5, 155, 90);
            // right eye
            draw_circle(hdc, 5, 95, 155, 180);

            let Some(mouse_pos) = POS.get() else {
                _ = unsafe { EndPaint(hwnd, &ps) };
                return LRESULT::default();
            };

            let mut rect = RECT::default();
            _ = unsafe { GetWindowRect(hwnd, &mut rect) };

            let center_of_left_eye = POINT {
                x: rect.left + 48,
                y: rect.top + 110,
            };
            let center_of_right_eye = POINT {
                x: center_of_left_eye.x + 90,
                y: center_of_left_eye.y,
            };

            // left iris
            draw_iris(hdc, mouse_pos, center_of_left_eye, 48.0);
            // right iris
            draw_iris(hdc, mouse_pos, center_of_right_eye, 138.0);

            _ = unsafe { EndPaint(hwnd, &ps) };
        }
        _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    LRESULT::default()
}

fn main() -> Result<()> {
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };

    unsafe { RegisterClassW(&wc) };

    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS_NAME,
            w!("xeyes"),
            WS_OVERLAPPED | WS_CAPTION | WS_VISIBLE | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            200,
            200,
            None,
            None,
            None,
            None,
        )?
    };

    unsafe { ShowWindow(hwnd, SW_SHOW).ok()? };
    unsafe { UpdateWindow(hwnd).ok()? };

    let mut msg = MSG::default();
    loop {
        if unsafe { !GetMessageW(&mut msg, None, 0, 0).as_bool() } {
            break;
        }
        _ = unsafe { TranslateMessage(&msg) };
        unsafe { DispatchMessageW(&msg) };
    }

    Ok(())
}
