use std::cell::Cell;
use std::ops::{Deref, DerefMut};

use anyhow::{Result, ensure};
use windows::{
    Win32::{
        Foundation::{HANDLE, HINSTANCE, HWND, INVALID_HANDLE_VALUE, LPARAM, LRESULT, WPARAM},
        System::{
            Memory::{
                CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
                OpenFileMappingW, PAGE_READWRITE, UnmapViewOfFile,
            },
            SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH},
        },
        UI::WindowsAndMessaging::{
            CallNextHookEx, HC_ACTION, HHOOK, SendMessageW, SetWindowsHookExW, WH_MOUSE_LL,
            WM_MOUSEMOVE, WM_USER,
        },
    },
    core::{Free, PCWSTR, w},
};

pub const NAME: PCWSTR = w!("XEyesMemoryMapObject");
pub const WM_HOOK_MOUSE_POS: u32 = WM_USER + 42;

thread_local! {
    static MAP_FILE: Cell<Option<HANDLE>> = const { Cell::new(None) };
    static HINST: Cell<Option<HINSTANCE>> = const { Cell::new(None) };
}

#[derive(Debug, Clone, Copy)]
struct ShareData {
    hook: HHOOK,
    hwnd: HWND,
}

struct MapView {
    handle: HANDLE,
    map_view_address: MEMORY_MAPPED_VIEW_ADDRESS,
}

impl MapView {
    fn new() -> Result<Self> {
        let handle = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS.0, false, NAME)? };

        let map_view_address =
            unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, size_of::<ShareData>()) };
        ensure!(!map_view_address.Value.is_null(), "failed to get map view.");

        Ok(Self {
            handle,
            map_view_address,
        })
    }
}

impl Deref for MapView {
    type Target = ShareData;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.map_view_address.Value as *const ShareData) }
    }
}

impl DerefMut for MapView {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.map_view_address.Value as *mut ShareData) }
    }
}

impl Drop for MapView {
    fn drop(&mut self) {
        unsafe { UnmapViewOfFile(self.map_view_address).ok() };
        unsafe { self.handle.free() };
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllMain(hinst: HINSTANCE, reason: u32, _: *const ()) -> bool {
    match reason {
        DLL_PROCESS_ATTACH => {
            HINST.set(Some(hinst));
            create_memory_map()
        }
        DLL_PROCESS_DETACH => delete_memory_map(),
        _ => Ok(()),
    }
    .is_ok()
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as _ && wparam.0 == WM_MOUSEMOVE as _ {
        let Ok(share_data) = MapView::new() else {
            return unsafe { CallNextHookEx(None, code, wparam, lparam) };
        };

        unsafe { SendMessageW(share_data.hwnd, WM_HOOK_MOUSE_POS, None, Some(lparam)) };
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

#[unsafe(no_mangle)]
pub fn set_hook(hwnd: HWND) -> bool {
    set_hook_impl(hwnd).is_ok()
}

fn set_hook_impl(hwnd: HWND) -> Result<()> {
    let mut share_data = MapView::new()?;
    share_data.hwnd = hwnd;

    let hhook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), HINST.get(), 0)? };

    share_data.hook = hhook;
    Ok(())
}

#[unsafe(no_mangle)]
pub fn end_hook() -> bool {
    end_hook_impl().is_ok()
}

fn end_hook_impl() -> Result<()> {
    let mut share_data = MapView::new()?;
    unsafe { share_data.hook.free() };
    Ok(())
}

fn create_memory_map() -> Result<()> {
    let hmapfile = unsafe {
        CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            None,
            PAGE_READWRITE,
            0,
            size_of::<ShareData>() as _,
            NAME,
        )?
    };
    MAP_FILE.set(Some(hmapfile));
    Ok(())
}

fn delete_memory_map() -> Result<()> {
    if let Some(mut mapfile) = MAP_FILE.get() {
        unsafe { mapfile.free() };
    }
    Ok(())
}
