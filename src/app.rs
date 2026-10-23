use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::{Fullscreen, WindowBuilder},
};
use tiny_http::Server;
use wry::{DragDropEvent, WebViewBuilder};
#[cfg(windows)]
use wry::WebViewBuilderExtWindows;

use crate::server;
use crate::state::{
    add_paths_to_playlist, load_path, load_playlist, save_playlist, State,
};
use crate::util::random_token;

fn load_window_icon() -> Option<tao::window::Icon> {
    let bytes = include_bytes!("../icon.ico");
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    tao::window::Icon::from_rgba(img.into_raw(), w, h).ok()
}

/// 单实例：已有进程则直接退出（Windows 命名互斥体）
#[cfg(windows)]
fn ensure_single_instance() -> bool {
    use std::sync::atomic::{AtomicIsize, Ordering};
    static HANDLE: AtomicIsize = AtomicIsize::new(0);
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateMutexW(lpMutexAttributes: usize, bInitialOwner: i32, lpName: *const u16) -> isize;
        fn GetLastError() -> u32;
        fn CloseHandle(h: isize) -> i32;
    }
    const ERROR_ALREADY_EXISTS: u32 = 183;
    let name: Vec<u16> = "Local\\rust-videos-single-instance\0"
        .encode_utf16()
        .collect();
    unsafe {
        let h = CreateMutexW(0, 1, name.as_ptr());
        if h == 0 {
            return true;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(h);
            return false;
        }
        HANDLE.store(h, Ordering::SeqCst);
        true
    }
}

#[cfg(not(windows))]
fn ensure_single_instance() -> bool {
    true
}

/// 标题栏/边框与页面背景 #1c1c1c 一致
#[cfg(windows)]
fn apply_window_chrome(window: &tao::window::Window) {
    use tao::platform::windows::WindowExtWindows;
    let hwnd = window.hwnd();
    const BG: u32 = 0x00_1C_1C_1C;
    const FG: u32 = 0x00_FF_FF_FF;
    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: isize,
            attr: u32,
            value: *const std::ffi::c_void,
            size: u32,
        ) -> i32;
    }
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    const DWMWA_BORDER_COLOR: u32 = 34;
    const DWMWA_CAPTION_COLOR: u32 = 35;
    const DWMWA_TEXT_COLOR: u32 = 36;
    let dark: u32 = 1;
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const _ as _, 4);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, &BG as *const _ as _, 4);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, &BG as *const _ as _, 4);
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_TEXT_COLOR, &FG as *const _ as _, 4);
    }
}

#[cfg(not(windows))]
fn apply_window_chrome(_window: &tao::window::Window) {}

pub fn run(initial: State) {
    if !ensure_single_instance() {
        // 已有实例在跑，直接退出
        return;
    }

    let state = Arc::new(Mutex::new(initial));
    let token = random_token();
    let server = Server::http("127.0.0.1:0").expect("无法启动本地服务");
    let port = server.server_addr().to_ip().expect("无效地址").port();
    {
        let state = Arc::clone(&state);
        let token = token.clone();
        thread::spawn(move || {
            for req in server.incoming_requests() {
                let state = Arc::clone(&state);
                let token = token.clone();
                thread::spawn(move || server::handle(req, &token, &state));
            }
        });
    }
    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let mut wb = WindowBuilder::new()
        .with_title("暴风影音")
        .with_inner_size(LogicalSize::new(1200.0, 800.0))
        .with_decorations(false)
        .with_theme(Some(tao::window::Theme::Dark));
    if let Some(icon) = load_window_icon() {
        wb = wb.with_window_icon(Some(icon));
    }
    let window = wb.build(&event_loop).expect("无法创建窗口");
    apply_window_chrome(&window);
    let data_dir = std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("rust-videos").join("webview2"));
    let mut web_context = wry::WebContext::new(data_dir);

    let drop_state = Arc::clone(&state);
    let drop_proxy = proxy.clone();

    let builder = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_background_color((0x1c, 0x1c, 0x1c, 255))
        .with_url(format!("http://127.0.0.1:{port}/{token}/"))
        .with_drag_drop_handler(move |event| {
            if let DragDropEvent::Drop { paths, .. } = event {
                if !paths.is_empty() {
                    let mut s = drop_state.lock().unwrap();
                    if add_paths_to_playlist(&mut s, &paths) {
                        save_playlist(&s);
                        let _ = drop_proxy.send_event("__refresh__".to_string());
                    }
                }
            }
            // true = 吃掉系统默认（Windows 上返回值本身被忽略，但设 handler 会接管拖放）
            true
        })
        .with_ipc_handler(move |req| {
            let body = req.body();
            if body == "quit" {
                let _ = proxy.send_event("__quit__".to_string());
            } else if body == "fullscreen-on" {
                let _ = proxy.send_event("__fullscreen_on__".to_string());
            } else if body == "fullscreen-off" {
                let _ = proxy.send_event("__fullscreen_off__".to_string());
            } else if body == "minimize" {
                let _ = proxy.send_event("__minimize__".to_string());
            } else if body == "maximize" {
                let _ = proxy.send_event("__maximize__".to_string());
            } else if body == "drag" {
                let _ = proxy.send_event("__drag__".to_string());
            } else if let Some(t) = body.strip_prefix("title:") {
                let _ = proxy.send_event(t.to_string());
            }
        });
    #[cfg(windows)]
    let builder = builder
        .with_additional_browser_args(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
             --autoplay-policy=no-user-gesture-required",
        )
        .with_browser_accelerator_keys(false);
    let webview = builder.build(&window).expect("无法创建 WebView");
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::UserEvent(msg) => {
                if msg == "__quit__" {
                    *control_flow = ControlFlow::Exit;
                } else if msg == "__fullscreen_on__" {
                    window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                } else if msg == "__fullscreen_off__" {
                    window.set_fullscreen(None);
                } else if msg == "__minimize__" {
                    window.set_minimized(true);
                } else if msg == "__maximize__" {
                    window.set_maximized(!window.is_maximized());
                } else if msg == "__drag__" {
                    let _ = window.drag_window();
                } else if msg == "__refresh__" {
                    // 拖入后刷新前端状态
                    let _ = webview.evaluate_script(
                        "window.refreshPlaylist&&window.refreshPlaylist()",
                    );
                } else {
                    window.set_title(&msg);
                }
            }
            _ => {}
        }
    });
}

pub fn initial_state_from_args(args: &[std::ffi::OsString]) -> State {
    match args.first().map(PathBuf::from) {
        Some(p) => {
            let s = load_path(&p).unwrap_or_else(State::empty);
            if !s.files.is_empty() {
                save_playlist(&s);
            }
            s
        }
        // 启动恢复播放列表，但不自动播放（前端 allowAutoplay=false）
        None => load_playlist().unwrap_or_else(State::empty),
    }
}
