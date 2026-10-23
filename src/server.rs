use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::Mutex,
};

use tiny_http::{Header, Method, Request, Response, StatusCode};

use crate::associations::reveal_in_explorer;
use crate::state::{
    add_file_to_playlist, add_folder_to_playlist, add_paths_to_playlist, clear_playlist,
    load_path, mime_of, pick_file_dialog, pick_folder_dialog, remove_at, remove_folder_node,
    reorder, reorder_in_folder, reorder_nodes, save_playlist, load_ui_prefs, save_ui_prefs, State,
};
use crate::util::{decode_base64, screenshots_dir};
use crate::video_edit;

const INDEX_HTML: &str = include_str!("index.html");
const APP_VERSION: &str = env!("APP_VERSION");

fn hdr(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).unwrap()
}

fn respond_json(req: Request, status: u16, body: String) {
    let resp = Response::from_string(body)
        .with_status_code(StatusCode(status))
        .with_header(hdr("Content-Type", "application/json; charset=utf-8"))
        .with_header(hdr("Cache-Control", "no-store"));
    let _ = req.respond(resp);
}

fn respond_status(req: Request, status: u16) {
    let _ = req.respond(Response::empty(StatusCode(status)));
}

fn parse_range(value: &str, total: u64) -> Option<(u64, u64)> {
    let spec = value.trim().strip_prefix("bytes=")?;
    let spec = spec.split(',').next()?.trim();
    let (a, b) = spec.split_once('-')?;
    if total == 0 {
        return None;
    }
    if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        if n == 0 {
            return None;
        }
        Some((total.saturating_sub(n), total - 1))
    } else {
        let start: u64 = a.parse().ok()?;
        let end: u64 = if b.is_empty() {
            total - 1
        } else {
            b.parse().ok()?
        };
        if start > end || start >= total {
            return None;
        }
        Some((start, end.min(total - 1)))
    }
}

fn serve_media(req: Request, path: PathBuf) {
    let mut file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return respond_status(req, 404),
    };
    let total = file.metadata().map(|m| m.len()).unwrap_or(0);
    let range_header = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("Range"))
        .map(|h| h.value.as_str().to_string());
    let (start, end, partial) = match range_header {
        Some(r) => match parse_range(&r, total) {
            Some((s, e)) => (s, e, true),
            None => {
                let resp = Response::empty(StatusCode(416))
                    .with_header(hdr("Content-Range", &format!("bytes */{total}")));
                let _ = req.respond(resp);
                return;
            }
        },
        None => (0, total.saturating_sub(1), false),
    };
    let len = if total == 0 { 0 } else { end - start + 1 };
    if file.seek(SeekFrom::Start(start)).is_err() {
        return respond_status(req, 500);
    }
    let mut headers = vec![
        hdr("Content-Type", mime_of(&path)),
        hdr("Accept-Ranges", "bytes"),
        hdr("Cache-Control", "no-store"),
    ];
    if partial {
        headers.push(hdr(
            "Content-Range",
            &format!("bytes {start}-{end}/{total}"),
        ));
    }
    let status = if partial { 206 } else { 200 };
    let resp = Response::new(
        StatusCode(status),
        headers,
        file.take(len),
        Some(len as usize),
        None,
    );
    let _ = req.respond(resp);
}

fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then_some(v)
    })
}

pub fn handle(mut req: Request, token: &str, state: &Mutex<State>) {
    let url = req.url().to_string();
    let (path, query) = url.split_once('?').unwrap_or((url.as_str(), ""));
    let prefix = format!("/{token}/");
    let Some(rest) = path.strip_prefix(prefix.as_str()) else {
        return respond_status(req, 403);
    };
    let method = req.method().clone();
    match (method, rest) {
        (Method::Get, "") => {
            let html = INDEX_HTML.replace("__APP_VERSION__", APP_VERSION);
            let resp = Response::from_string(html)
                .with_header(hdr("Content-Type", "text/html; charset=utf-8"))
                .with_header(hdr("Cache-Control", "no-store"));
            let _ = req.respond(resp);
        }
        (Method::Get, "state") => {
            let body = state.lock().unwrap().to_json();
            respond_json(req, 200, body);
        }
        (Method::Get, "ui-prefs") => {
            let body = load_ui_prefs().to_string();
            respond_json(req, 200, body);
        }
        (Method::Post, "ui-prefs") => {
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_else(|_| serde_json::json!({}));
            save_ui_prefs(&v);
            respond_json(req, 200, serde_json::json!({ "ok": true }).to_string());
        }
        (Method::Get, r) if r.starts_with("media/") => {
            // media/0 或 media/0?p=绝对路径（排序后下标会变，路径优先）
            let id_part = r["media/".len()..].split('?').next().unwrap_or("");
            let id: Option<usize> = id_part.parse().ok();
            let path_q = query_param(query, "p").map(|s| {
                // percent-decode minimal
                let s = s.replace('+', " ");
                let mut out = String::new();
                let b = s.as_bytes();
                let mut i = 0;
                while i < b.len() {
                    if b[i] == b'%' && i + 2 < b.len() {
                        let h = |c: u8| -> Option<u8> {
                            match c {
                                b'0'..=b'9' => Some(c - b'0'),
                                b'a'..=b'f' => Some(c - b'a' + 10),
                                b'A'..=b'F' => Some(c - b'A' + 10),
                                _ => None,
                            }
                        };
                        if let (Some(a), Some(c)) = (h(b[i+1]), h(b[i+2])) {
                            out.push((a << 4 | c) as char);
                            i += 3;
                            continue;
                        }
                    }
                    out.push(b[i] as char);
                    i += 1;
                }
                PathBuf::from(out)
            });
            let path = if let Some(p) = path_q {
                if p.is_file() {
                    Some(p)
                } else {
                    id.and_then(|i| state.lock().unwrap().files.get(i).cloned())
                }
            } else {
                id.and_then(|i| state.lock().unwrap().files.get(i).cloned())
            };
            match path {
                Some(p) => serve_media(req, p),
                None => respond_status(req, 404),
            }
        }
        (Method::Get, "ffmpeg-status") => {
            respond_json(
                req,
                200,
                serde_json::json!({ "available": video_edit::available() }).to_string(),
            );
        }
        (Method::Post, "delete") => {
            let i = query_param(query, "i")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or_else(|| state.lock().unwrap().current);
            let mut s = state.lock().unwrap();
            if i >= s.files.len() {
                return respond_json(
                    req,
                    404,
                    serde_json::json!({ "error": "文件不存在" }).to_string(),
                );
            }
            // 只从播放列表移除，绝不删除本地文件
            if remove_at(&mut s, i) {
                save_playlist(&s);
                let body = s.to_json();
                drop(s);
                respond_json(req, 200, body);
            } else {
                drop(s);
                respond_json(
                    req,
                    404,
                    serde_json::json!({ "error": "列表项不存在" }).to_string(),
                );
            }
        }
        (Method::Post, "open") => {
            // 打开文件：挂到播放列表根节点，已存在则选中并播放
            if let Some(p) = pick_file_dialog() {
                let mut s = state.lock().unwrap();
                if add_file_to_playlist(&mut s, p) {
                    save_playlist(&s);
                }
                let body = s.to_json();
                drop(s);
                respond_json(req, 200, body);
            } else {
                let body = state.lock().unwrap().to_json();
                respond_json(req, 200, body);
            }
        }
        (Method::Post, "open-folder") => {
            // 打开文件夹：根节点加【文件夹名】，递归视频全挂其下（禁止套目录）
            if let Some(p) = pick_folder_dialog() {
                let mut s = state.lock().unwrap();
                if add_folder_to_playlist(&mut s, &p) {
                    save_playlist(&s);
                }
                let body = s.to_json();
                drop(s);
                respond_json(req, 200, body);
            } else {
                let body = state.lock().unwrap().to_json();
                respond_json(req, 200, body);
            }
        }
        (Method::Post, "delete-folder") => {
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let node = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("node").and_then(|x| x.as_u64()))
                .map(|n| n as usize);
            let mut s = state.lock().unwrap();
            if let Some(ni) = node {
                if remove_folder_node(&mut s, ni) {
                    save_playlist(&s);
                }
            }
            let body = s.to_json();
            drop(s);
            respond_json(req, 200, body);
        }
        (Method::Get, "props") => {
            let i = query_param(query, "i")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or_else(|| state.lock().unwrap().current);
            let s = state.lock().unwrap();
            let path = s.files.get(i).cloned();
            drop(s);
            match path {
                Some(p) => {
                    let meta = fs::metadata(&p).ok();
                    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let parent = p
                        .parent()
                        .map(|x| x.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let ext = p
                        .extension()
                        .map(|e| e.to_string_lossy().to_ascii_lowercase())
                        .unwrap_or_default();
                    respond_json(
                        req,
                        200,
                        serde_json::json!({
                            "name": name,
                            "path": p.to_string_lossy(),
                            "dir": parent,
                            "size": size,
                            "ext": ext,
                            "kind": "video",
                            "type_label": "视频",
                        })
                        .to_string(),
                    );
                }
                None => respond_json(
                    req,
                    404,
                    serde_json::json!({ "error": "无当前文件" }).to_string(),
                ),
            }
        }
        (Method::Post, "clear") => {
            let mut s = state.lock().unwrap();
            clear_playlist(&mut s);
            save_playlist(&s);
            let body = s.to_json();
            drop(s);
            respond_json(req, 200, body);
        }
        (Method::Post, "reorder") => {
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            let kind = v.get("kind").and_then(|x| x.as_str()).unwrap_or("file");
            let mut s = state.lock().unwrap();
            match kind {
                "node" => {
                    let from = v.get("from").and_then(|x| x.as_u64()).map(|n| n as usize);
                    let to = v.get("to").and_then(|x| x.as_u64()).map(|n| n as usize);
                    if let (Some(f), Some(t)) = (from, to) {
                        reorder_nodes(&mut s, f, t);
                        save_playlist(&s);
                    }
                }
                "folder" => {
                    let node = v.get("node").and_then(|x| x.as_u64()).map(|n| n as usize);
                    let from = v.get("from").and_then(|x| x.as_u64()).map(|n| n as usize);
                    let to = v.get("to").and_then(|x| x.as_u64()).map(|n| n as usize);
                    if let (Some(n), Some(f), Some(t)) = (node, from, to) {
                        reorder_in_folder(&mut s, n, f, t);
                        save_playlist(&s);
                    }
                }
                _ => {
                    let from = v.get("from").and_then(|x| x.as_u64()).map(|n| n as usize);
                    let to = v.get("to").and_then(|x| x.as_u64()).map(|n| n as usize);
                    if let (Some(f), Some(t)) = (from, to) {
                        reorder(&mut s, f, t);
                        save_playlist(&s);
                    }
                }
            }
            let body = s.to_json();
            drop(s);
            respond_json(req, 200, body);
        }
        (Method::Post, "save-list") => {
            // 导出播放列表为 JSON（用户选路径）
            let s = state.lock().unwrap();
            let json = s.to_json();
            drop(s);
            if let Some(path) = rfd::FileDialog::new()
                .set_title("保存播放列表")
                .add_filter("JSON", &["json"])
                .set_file_name("playlist.json")
                .save_file()
            {
                match fs::write(&path, json.as_bytes()) {
                    Ok(()) => respond_json(
                        req,
                        200,
                        serde_json::json!({ "ok": true, "path": path.to_string_lossy() }).to_string(),
                    ),
                    Err(e) => respond_json(
                        req,
                        500,
                        serde_json::json!({ "error": format!("保存失败：{e}") }).to_string(),
                    ),
                }
            } else {
                respond_json(req, 200, serde_json::json!({ "ok": false, "cancelled": true }).to_string());
            }
        }
        (Method::Post, "drop-paths") => {
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let raw: Vec<String> = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("paths").cloned())
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            let paths: Vec<PathBuf> = raw.into_iter().map(PathBuf::from).collect();
            let mut s = state.lock().unwrap();
            if add_paths_to_playlist(&mut s, &paths) {
                save_playlist(&s);
            }
            let body = s.to_json();
            drop(s);
            respond_json(req, 200, body);
        }
        (Method::Post, "nav") => {
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let delta = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("delta").and_then(|x| x.as_i64()))
                .unwrap_or(0);
            let mut s = state.lock().unwrap();
            if !s.files.is_empty() {
                let n = s.current as i64 + delta;
                if n >= 0 && (n as usize) < s.files.len() {
                    s.current = n as usize;
                }
            }
            save_playlist(&s);
            let body = s.to_json();
            drop(s);
            respond_json(req, 200, body);
        }
        (Method::Post, "goto") => {
            let mut body = String::new();
            let _ = req.as_reader().read_to_string(&mut body);
            let index = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("index").and_then(|x| x.as_u64()))
                .map(|n| n as usize);
            let mut s = state.lock().unwrap();
            if let Some(i) = index {
                if i < s.files.len() {
                    s.current = i;
                }
            }
            save_playlist(&s);
            let body = s.to_json();
            drop(s);
            respond_json(req, 200, body);
        }
        (Method::Post, "screenshot") => {
            let mut body = String::new();
            if req.as_reader().read_to_string(&mut body).is_err() {
                return respond_json(
                    req,
                    400,
                    serde_json::json!({ "error": "无法读取截图数据" }).to_string(),
                );
            }
            let body = body.trim();
            let (b64, file_stem) = if body.starts_with('{') {
                match serde_json::from_str::<serde_json::Value>(body) {
                    Ok(v) => {
                        let data = v
                            .get("data")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = v
                            .get("filename")
                            .and_then(|x| x.as_str())
                            .unwrap_or("截图")
                            .to_string();
                        (data, name)
                    }
                    Err(_) => {
                        return respond_json(
                            req,
                            400,
                            serde_json::json!({ "error": "截图数据无效" }).to_string(),
                        );
                    }
                }
            } else {
                let t = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                (body.to_string(), format!("Screenshot_{t}"))
            };
            let Some(png) = decode_base64(&b64) else {
                return respond_json(
                    req,
                    400,
                    serde_json::json!({ "error": "截图数据无效" }).to_string(),
                );
            };
            if png.is_empty() {
                return respond_json(
                    req,
                    400,
                    serde_json::json!({ "error": "截图为空" }).to_string(),
                );
            }
            let safe: String = file_stem
                .chars()
                .map(|c| match c {
                    '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                    _ => c,
                })
                .collect();
            let safe = safe.trim().trim_matches('.').to_string();
            let safe = if safe.is_empty() {
                "截图".to_string()
            } else {
                safe
            };
            let dir = screenshots_dir();
            let path = dir.join(format!("{safe}.png"));
            match fs::write(&path, &png) {
                Ok(()) => respond_json(
                    req,
                    200,
                    serde_json::json!({ "path": path.to_string_lossy() }).to_string(),
                ),
                Err(e) => respond_json(
                    req,
                    500,
                    serde_json::json!({ "error": format!("保存失败：{e}") }).to_string(),
                ),
            }
        }
        (Method::Post, r) if r.starts_with("reveal") => {
            // reveal?i=文件下标 或 reveal?node=文件夹节点下标
            let node = query_param(query, "node").and_then(|v| v.parse::<usize>().ok());
            let path = {
                let s = state.lock().unwrap();
                if let Some(ni) = node {
                    match s.nodes.get(ni) {
                        Some(crate::state::PlaylistNode::Folder { path, .. }) => Some(path.clone()),
                        Some(crate::state::PlaylistNode::File(p)) => Some(p.clone()),
                        None => None,
                    }
                } else {
                    let i = query_param(query, "i")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(s.current);
                    s.files.get(i).cloned()
                }
            };
            match path {
                Some(p) => {
                    // 文件 / 文件夹统一：在父目录中选中该项（explorer /select）
                    match reveal_in_explorer(&p) {
                        Ok(()) => respond_json(
                            req,
                            200,
                            serde_json::json!({ "ok": true }).to_string(),
                        ),
                        Err(e) => respond_json(
                            req,
                            500,
                            serde_json::json!({ "error": e }).to_string(),
                        ),
                    }
                }
                None => respond_json(
                    req,
                    404,
                    serde_json::json!({ "error": "路径不存在" }).to_string(),
                ),
            }
        }
        (Method::Post, "edit/trim") => {
            let mut body = String::new();
            if req.as_reader().read_to_string(&mut body).is_err() {
                return respond_json(
                    req,
                    400,
                    serde_json::json!({ "error": "无法读取参数" }).to_string(),
                );
            }
            let v: serde_json::Value = match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(_) => {
                    return respond_json(
                        req,
                        400,
                        serde_json::json!({ "error": "请求参数无效" }).to_string(),
                    )
                }
            };
            let i = v
                .get("i")
                .and_then(|x| x.as_u64())
                .map(|n| n as usize)
                .unwrap_or_else(|| state.lock().unwrap().current);
            let start = v.get("start").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let end = v.get("end").and_then(|x| x.as_f64());
            let duration = v.get("duration").and_then(|x| x.as_f64());
            let src = match state.lock().unwrap().files.get(i).cloned() {
                Some(p) => p,
                None => {
                    return respond_json(
                        req,
                        404,
                        serde_json::json!({ "error": "文件不存在" }).to_string(),
                    )
                }
            };
            match video_edit::trim(&src, start, end, duration) {
                Ok(out) => respond_json(
                    req,
                    200,
                    serde_json::json!({ "path": out.to_string_lossy() }).to_string(),
                ),
                Err(e) => respond_json(req, 500, serde_json::json!({ "error": e }).to_string()),
            }
        }
        (Method::Post, "edit/bitrate") => {
            let mut body = String::new();
            if req.as_reader().read_to_string(&mut body).is_err() {
                return respond_json(
                    req,
                    400,
                    serde_json::json!({ "error": "无法读取参数" }).to_string(),
                );
            }
            let v: serde_json::Value = match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(_) => {
                    return respond_json(
                        req,
                        400,
                        serde_json::json!({ "error": "请求参数无效" }).to_string(),
                    )
                }
            };
            let i = v
                .get("i")
                .and_then(|x| x.as_u64())
                .map(|n| n as usize)
                .unwrap_or_else(|| state.lock().unwrap().current);
            let kbps = v
                .get("bitrate")
                .or_else(|| v.get("kbps"))
                .and_then(|x| x.as_i64())
                .unwrap_or(3000);
            let src = match state.lock().unwrap().files.get(i).cloned() {
                Some(p) => p,
                None => {
                    return respond_json(
                        req,
                        404,
                        serde_json::json!({ "error": "文件不存在" }).to_string(),
                    )
                }
            };
            match video_edit::bitrate(&src, kbps) {
                Ok(out) => respond_json(
                    req,
                    200,
                    serde_json::json!({ "path": out.to_string_lossy() }).to_string(),
                ),
                Err(e) => respond_json(req, 500, serde_json::json!({ "error": e }).to_string()),
            }
        }
        (Method::Post, "edit/split") => {
            let mut body = String::new();
            if req.as_reader().read_to_string(&mut body).is_err() {
                return respond_json(
                    req,
                    400,
                    serde_json::json!({ "error": "无法读取参数" }).to_string(),
                );
            }
            let v: serde_json::Value = match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(_) => {
                    return respond_json(
                        req,
                        400,
                        serde_json::json!({ "error": "请求参数无效" }).to_string(),
                    )
                }
            };
            let i = v
                .get("i")
                .and_then(|x| x.as_u64())
                .map(|n| n as usize)
                .unwrap_or_else(|| state.lock().unwrap().current);
            let side = v.get("side").and_then(|x| x.as_str()).unwrap_or("both");
            let src = match state.lock().unwrap().files.get(i).cloned() {
                Some(p) => p,
                None => {
                    return respond_json(
                        req,
                        404,
                        serde_json::json!({ "error": "文件不存在" }).to_string(),
                    )
                }
            };
            match video_edit::split_h(&src, side) {
                Ok(outs) => {
                    let paths: Vec<_> = outs
                        .iter()
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect();
                    respond_json(
                        req,
                        200,
                        serde_json::json!({ "paths": paths, "path": paths.first() }).to_string(),
                    );
                }
                Err(e) => respond_json(req, 500, serde_json::json!({ "error": e }).to_string()),
            }
        }
        (Method::Post, "edit/aspect") => {
            let mut body = String::new();
            if req.as_reader().read_to_string(&mut body).is_err() {
                return respond_json(
                    req,
                    400,
                    serde_json::json!({ "error": "无法读取参数" }).to_string(),
                );
            }
            let v: serde_json::Value = match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(_) => {
                    return respond_json(
                        req,
                        400,
                        serde_json::json!({ "error": "请求参数无效" }).to_string(),
                    )
                }
            };
            let i = v
                .get("i")
                .and_then(|x| x.as_u64())
                .map(|n| n as usize)
                .unwrap_or_else(|| state.lock().unwrap().current);
            let ratio = v.get("ratio").and_then(|x| x.as_str()).unwrap_or("16:9");
            let mode = v.get("mode").and_then(|x| x.as_str()).unwrap_or("stretch");
            let src = match state.lock().unwrap().files.get(i).cloned() {
                Some(p) => p,
                None => {
                    return respond_json(
                        req,
                        404,
                        serde_json::json!({ "error": "文件不存在" }).to_string(),
                    )
                }
            };
            match video_edit::change_aspect(&src, ratio, mode) {
                Ok(out) => respond_json(
                    req,
                    200,
                    serde_json::json!({ "path": out.to_string_lossy() }).to_string(),
                ),
                Err(e) => respond_json(req, 500, serde_json::json!({ "error": e }).to_string()),
            }
        }
        _ => respond_status(req, 404),
    }
}
