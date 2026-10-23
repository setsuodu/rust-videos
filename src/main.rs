#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod associations;
mod server;
mod state;
mod util;
mod video_edit;

use associations::{register_associations, unregister_associations};
use util::show_console_message;

fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if let Some(first) = args.first().and_then(|a| a.to_str()) {
        match first {
            "--register" | "/register" => {
                match register_associations() {
                    Ok(()) => show_console_message(
                        "已注册为视频打开方式（类型显示真实后缀，如「MP4 文件」）。",
                    ),
                    Err(e) => show_console_message(&format!("注册失败：{e}")),
                }
                return;
            }
            "--unregister" | "/unregister" => {
                match unregister_associations() {
                    Ok(()) => show_console_message("已取消文件关联注册。"),
                    Err(e) => show_console_message(&format!("取消注册失败：{e}")),
                }
                return;
            }
            _ => {}
        }
    }
    let initial = app::initial_state_from_args(&args);
    app::run(initial);
}
