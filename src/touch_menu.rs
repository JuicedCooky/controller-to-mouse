// Owner-drawn Win32 popup menu with large, touch-friendly items.
// The standard tray menu (muda) uses the system's compact item height,
// which is hard to hit with a finger.

use std::cell::{Cell, RefCell};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, DeleteObject, DrawTextW, FillRect, GetDC, GetSysColor, GetSysColorBrush,
    GetTextExtentPoint32W, ReleaseDC, SelectObject, SetBkMode, SetTextColor, COLOR_HIGHLIGHT,
    COLOR_HIGHLIGHTTEXT, COLOR_MENU, COLOR_MENUTEXT, DT_LEFT, DT_SINGLELINE, DT_VCENTER, HDC,
    HFONT, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, MEASUREITEMSTRUCT, ODS_SELECTED, ODT_MENU};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, PostMessageW,
    RegisterClassW, SetForegroundWindow, TrackPopupMenuEx, HMENU,
    MF_OWNERDRAW, TPM_BOTTOMALIGN, TPM_RETURNCMD,
    TPM_RIGHTALIGN, TPM_RIGHTBUTTON, WM_DRAWITEM, WM_MEASUREITEM, WM_NULL, WNDCLASSW,
    WS_EX_TOOLWINDOW, WS_POPUP,
};

// Sizes in 96-DPI pixels, scaled to the actual DPI at draw time.
#[derive(Clone, Copy)]
struct Metrics {
    item_height: i32,
    font_height: i32,
    pad_x: i32,
    min_width: i32,
}

const LARGE: Metrics = Metrics { item_height: 56, font_height: 22, pad_x: 28, min_width: 220 };
const NORMAL: Metrics = Metrics { item_height: 30, font_height: 15, pad_x: 16, min_width: 140 };

thread_local! {
    static OWNER: Cell<HWND> = Cell::new(HWND(0));
    static LABELS: RefCell<Vec<(u32, Vec<u16>)>> = RefCell::new(Vec::new());
    static METRICS: Cell<Metrics> = Cell::new(LARGE);
}

fn metrics() -> Metrics {
    METRICS.with(|m| m.get())
}

fn scale(px: i32, dpi: u32) -> i32 {
    px * dpi as i32 / 96
}

fn dpi(hwnd: HWND) -> u32 {
    match unsafe { GetDpiForWindow(hwnd) } {
        0 => 96,
        d => d,
    }
}

fn label_for(id: u32) -> Vec<u16> {
    LABELS.with(|l| {
        l.borrow()
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, s)| s.clone())
            .unwrap_or_default()
    })
}

unsafe fn make_font(dpi: u32) -> HFONT {
    CreateFontW(
        -scale(metrics().font_height, dpi), 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 5, 0,
        w!("Segoe UI"),
    )
}

unsafe fn text_width(hdc: HDC, text: &[u16]) -> i32 {
    let mut size = SIZE::default();
    let _ = GetTextExtentPoint32W(hdc, text, &mut size);
    size.cx
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MEASUREITEM => {
            let mis = &mut *(lparam.0 as *mut MEASUREITEMSTRUCT);
            if mis.CtlType != ODT_MENU {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let dpi = dpi(hwnd);
            let hdc = GetDC(hwnd);
            let font = make_font(dpi);
            let old = SelectObject(hdc, font);
            let w = text_width(hdc, &label_for(mis.itemID));
            SelectObject(hdc, old);
            let _ = DeleteObject(font);
            ReleaseDC(hwnd, hdc);

            let m = metrics();
            mis.itemWidth = (w + scale(m.pad_x, dpi) * 2).max(scale(m.min_width, dpi)) as u32;
            mis.itemHeight = scale(m.item_height, dpi) as u32;
            LRESULT(1)
        }
        WM_DRAWITEM => {
            let dis = &*(lparam.0 as *const DRAWITEMSTRUCT);
            if dis.CtlType != ODT_MENU {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let dpi = dpi(hwnd);
            let selected = (dis.itemState.0 & ODS_SELECTED.0) != 0;
            let (bg, fg) = if selected {
                (COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT)
            } else {
                (COLOR_MENU, COLOR_MENUTEXT)
            };

            FillRect(dis.hDC, &dis.rcItem, GetSysColorBrush(bg));
            SetBkMode(dis.hDC, TRANSPARENT);
            SetTextColor(dis.hDC, windows::Win32::Foundation::COLORREF(GetSysColor(fg)));

            let font = make_font(dpi);
            let old = SelectObject(dis.hDC, font);
            let mut rect = RECT { left: dis.rcItem.left + scale(metrics().pad_x, dpi), ..dis.rcItem };
            let mut text = label_for(dis.itemID);
            DrawTextW(dis.hDC, &mut text, &mut rect, DT_LEFT | DT_SINGLELINE | DT_VCENTER);
            SelectObject(dis.hDC, old);
            let _ = DeleteObject(font);
            LRESULT(1)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// Hidden window that owns the popup and receives WM_MEASUREITEM / WM_DRAWITEM.
fn owner_window() -> HWND {
    OWNER.with(|o| {
        if o.get().0 != 0 {
            return o.get();
        }
        unsafe {
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let class = w!("ControllerTrayTouchMenu");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class,
                PCWSTR::null(),
                WS_POPUP,
                0, 0, 0, 0,
                None,
                None,
                instance,
                None,
            );
            o.set(hwnd);
            hwnd
        }
    })
}

/// Shows a popup menu (large touch-sized items, or normal-sized) at the given
/// screen position and blocks until the user picks an item or dismisses it.
/// Returns the chosen id.
pub fn show(items: &[(u32, &str)], x: i32, y: i32, large: bool) -> Option<u32> {
    let hwnd = owner_window();
    METRICS.with(|m| m.set(if large { LARGE } else { NORMAL }));
    LABELS.with(|l| {
        *l.borrow_mut() = items
            .iter()
            .map(|(id, s)| (*id, s.encode_utf16().collect()))
            .collect();
    });

    unsafe {
        let menu: HMENU = CreatePopupMenu().ok()?;
        for (id, _) in items {
            let _ = AppendMenuW(menu, MF_OWNERDRAW, *id as usize, PCWSTR::null());
        }

        // Required so the menu closes when the user taps elsewhere.
        let _ = SetForegroundWindow(hwnd);
        let pt = POINT { x, y };
        let cmd = TrackPopupMenuEx(
            menu,
            (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_RIGHTALIGN | TPM_BOTTOMALIGN).0,
            pt.x,
            pt.y,
            hwnd,
            None,
        );
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);

        match cmd.0 {
            0 => None,
            id => Some(id as u32),
        }
    }
}
