use std::{
    fs,
    path::{Path, PathBuf},
};

pub const VIDEO_EXTS: &[&str] = &[
    "mp4", "m4v", "webm", "mov", "mkv", "ogv", "avi", "wmv", "flv", "ts", "m2ts", "mpeg",
    "mpg", "3gp", "asf", "rm", "rmvb",
];

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Video,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Video => "video",
        }
    }
}

pub fn ext_of(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// 仅视频；不再打开图片
pub fn kind_of(p: &Path) -> Option<Kind> {
    let e = ext_of(p);
    if VIDEO_EXTS.contains(&e.as_str()) {
        Some(Kind::Video)
    } else {
        None
    }
}

pub fn mime_of(p: &Path) -> &'static str {
    match ext_of(p).as_str() {
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "ogv" => "video/ogg",
        "avi" => "video/x-msvideo",
        "wmv" | "asf" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        "ts" | "m2ts" => "video/mp2t",
        "mpeg" | "mpg" => "video/mpeg",
        "3gp" => "video/3gpp",
        _ => "application/octet-stream",
    }
}

/// 播放列表节点：根下要么是单文件，要么是【文件夹名】分组（内部禁止再套目录）
#[derive(Clone)]
pub enum PlaylistNode {
    /// 打开文件：直接挂在根节点
    File(PathBuf),
    /// 打开文件夹：根上一个文件夹节点，下面只挂该文件夹递归扫出的视频（扁平）
    Folder {
        name: String,
        /// 打开时的文件夹绝对路径（用于「打开位置」）
        path: PathBuf,
        /// 绝对路径，递归收集、已排序、无子目录层级
        files: Vec<PathBuf>,
    },
}

pub struct State {
    /// 最近一次有意义的目录（保存用）
    pub dir: PathBuf,
    /// 显示树：根节点列表
    pub nodes: Vec<PlaylistNode>,
    /// 扁平播放序列（与 nodes 同步，供 media/goto/nav/delete 使用）
    pub files: Vec<PathBuf>,
    pub current: usize,
}

impl State {
    pub fn empty() -> Self {
        State {
            dir: PathBuf::new(),
            nodes: Vec::new(),
            files: Vec::new(),
            current: 0,
        }
    }

    /// 根据 nodes 重建扁平 files，并夹紧 current
    pub fn rebuild_files(&mut self) {
        self.files.clear();
        for n in &self.nodes {
            match n {
                PlaylistNode::File(p) => self.files.push(p.clone()),
                PlaylistNode::Folder { files, .. } => {
                    self.files.extend(files.iter().cloned());
                }
            }
        }
        if self.files.is_empty() {
            self.current = 0;
        } else if self.current >= self.files.len() {
            self.current = self.files.len() - 1;
        }
    }

    pub fn to_json(&self) -> String {
        // 扁平 files（播放用）
        let files_json: Vec<_> = self
            .files
            .iter()
            .map(|p| {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let path = p.to_string_lossy().into_owned();
                let missing = !p.is_file();
                serde_json::json!({
                    "name": name,
                    "path": path,
                    "kind": kind_of(p).map(Kind::as_str).unwrap_or("video"),
                    "missing": missing,
                })
            })
            .collect();

        // 树 nodes（显示用），每个叶子带全局 index
        let mut idx = 0usize;
        let mut nodes_json = Vec::new();
        for n in &self.nodes {
            match n {
                PlaylistNode::File(p) => {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    nodes_json.push(serde_json::json!({
                        "type": "file",
                        "name": name,
                        "path": p.to_string_lossy(),
                        "index": idx,
                        "kind": "video",
                        "missing": !p.is_file(),
                    }));
                    idx += 1;
                }
                PlaylistNode::Folder { name, path, files } => {
                    let children: Vec<_> = files
                        .iter()
                        .map(|p| {
                            let fname = p
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            let j = serde_json::json!({
                                "name": fname,
                                "path": p.to_string_lossy(),
                                "index": idx,
                                "kind": "video",
                                "missing": !p.is_file(),
                            });
                            idx += 1;
                            j
                        })
                        .collect();
                    nodes_json.push(serde_json::json!({
                        "type": "folder",
                        "name": name,
                        "path": path.to_string_lossy(),
                        "count": files.len(),
                        "children": children,
                    }));
                }
            }
        }

        serde_json::json!({
            "dir": self.dir.to_string_lossy(),
            "files": files_json,
            "nodes": nodes_json,
            "current": self.current,
        })
        .to_string()
    }
}

/// %LOCALAPPDATA%/rust-videos/playlist.json
pub fn playlist_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("rust-videos").join("playlist.json")
}

/// 自动保存：绝对路径扁平列表 + current + nodes
pub fn save_playlist(state: &State) {
    let path = playlist_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let files: Vec<String> = state
        .files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let mut nodes_ser = Vec::new();
    for n in &state.nodes {
        match n {
            PlaylistNode::File(p) => {
                nodes_ser.push(serde_json::json!({
                    "type": "file",
                    "path": p.to_string_lossy(),
                }));
            }
            PlaylistNode::Folder { name, path, files } => {
                let paths: Vec<_> = files
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                nodes_ser.push(serde_json::json!({
                    "type": "folder",
                    "name": name,
                    "path": path.to_string_lossy(),
                    "files": paths,
                }));
            }
        }
    }
    let body = serde_json::json!({
        "dir": state.dir.to_string_lossy(),
        "files": files,
        "nodes": nodes_ser,
        "current": state.current,
    });
    let _ = fs::write(path, body.to_string());
}

/// 启动时恢复；已不存在的文件跳过
pub fn load_playlist() -> Option<State> {
    let path = playlist_path();
    let raw = fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let dir = v
        .get("dir")
        .and_then(|x| x.as_str())
        .map(PathBuf::from)
        .unwrap_or_default();

    let mut nodes = Vec::new();
    if let Some(arr) = v.get("nodes").and_then(|x| x.as_array()) {
        for item in arr {
            let ty = item.get("type").and_then(|x| x.as_str()).unwrap_or("");
            if ty == "folder" {
                let name = item
                    .get("name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("文件夹")
                    .to_string();
                let mut files = Vec::new();
                if let Some(list) = item.get("files").and_then(|x| x.as_array()) {
                    for f in list {
                        let p = f.as_str().map(PathBuf::from).unwrap_or_default();
                        // 保留已删除路径，前端标 ❌ 并跳过播放
                        if !p.as_os_str().is_empty()
                            && (kind_of(&p).is_some() || p.extension().is_some())
                        {
                            files.push(p);
                        }
                    }
                }
                if !files.is_empty() {
                    let fpath = item
                        .get("path")
                        .and_then(|x| x.as_str())
                        .map(PathBuf::from)
                        .unwrap_or_else(|| {
                            files[0]
                                .parent()
                                .map(|p| p.to_path_buf())
                                .unwrap_or_default()
                        });
                    nodes.push(PlaylistNode::Folder {
                        name,
                        path: fpath,
                        files,
                    });
                }
            } else {
                let p = item
                    .get("path")
                    .and_then(|x| x.as_str())
                    .map(PathBuf::from)
                    .unwrap_or_default();
                if !p.as_os_str().is_empty()
                    && (kind_of(&p).is_some() || p.extension().is_some())
                {
                    nodes.push(PlaylistNode::File(p));
                }
            }
        }
    } else if let Some(list) = v.get("files").and_then(|x| x.as_array()) {
        // 旧格式：全扁平
        for item in list {
            let p = match item {
                serde_json::Value::String(s) => PathBuf::from(s),
                other => other
                    .get("path")
                    .and_then(|x| x.as_str())
                    .map(PathBuf::from)
                    .unwrap_or_default(),
            };
            if !p.as_os_str().is_empty()
                && (kind_of(&p).is_some() || p.extension().is_some())
            {
                nodes.push(PlaylistNode::File(p));
            }
        }
    }

    if nodes.is_empty() {
        return None;
    }
    let mut state = State {
        dir,
        nodes,
        files: Vec::new(),
        current: 0,
    };
    state.rebuild_files();
    let current = v
        .get("current")
        .and_then(|x| x.as_u64())
        .map(|n| n as usize)
        .unwrap_or(0)
        .min(state.files.len().saturating_sub(1));
    state.current = current;
    Some(state)
}

/// 递归收集目录下所有视频，扁平，不保留子目录层级（禁止套目录）
fn collect_media_recursive(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('.')
                        || name.eq_ignore_ascii_case("$RECYCLE.BIN")
                        || name.eq_ignore_ascii_case("System Volume Information")
                    {
                        continue;
                    }
                }
                walk(&p, out);
            } else if p.is_file() && kind_of(&p).is_some() {
                out.push(p);
            }
        }
    }
    walk(root, &mut out);
    out
}

fn sort_media(files: &mut [PathBuf]) {
    files.sort_by(|a, b| {
        let an = a.to_string_lossy().to_lowercase();
        let bn = b.to_string_lossy().to_lowercase();
        natord::compare(&an, &bn)
    });
}

fn same_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (dunce::canonicalize(a), dunce::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// CLI / 关联打开：单文件 → 根上一个 File 节点；文件夹 → 一个 Folder 节点
pub fn load_path(path: &Path) -> Option<State> {
    if path.is_dir() {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        let mut files = collect_media_recursive(path);
        if files.is_empty() {
            return None;
        }
        sort_media(&mut files);
        let mut state = State {
            dir: path.to_path_buf(),
            nodes: vec![PlaylistNode::Folder {
                name,
                path: path.to_path_buf(),
                files,
            }],
            files: Vec::new(),
            current: 0,
        };
        state.rebuild_files();
        return Some(state);
    }

    if !path.is_file() || kind_of(path).is_none() {
        return None;
    }
    let abs = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = abs.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut state = State {
        dir,
        nodes: vec![PlaylistNode::File(abs)],
        files: Vec::new(),
        current: 0,
    };
    state.rebuild_files();
    Some(state)
}

/// 【打开文件】：新文件挂到播放列表根节点；已在列表则只选中并播放
pub fn add_file_to_playlist(state: &mut State, path: PathBuf) -> bool {
    if kind_of(&path).is_none() || !path.is_file() {
        return false;
    }
    let path = dunce::canonicalize(&path).unwrap_or(path);

    if let Some(i) = state.files.iter().position(|f| same_path(f, &path)) {
        state.current = i;
        return true;
    }

    state.nodes.push(PlaylistNode::File(path.clone()));
    if state.dir.as_os_str().is_empty() {
        if let Some(parent) = path.parent() {
            state.dir = parent.to_path_buf();
        }
    }
    state.rebuild_files();
    state.current = state.files.len().saturating_sub(1);
    true
}

/// 【打开文件夹】：根节点添加【文件夹名】节点；递归视频全部挂在该节点下（禁止套目录）
pub fn add_folder_to_playlist(state: &mut State, root: &Path) -> bool {
    if !root.is_dir() {
        return false;
    }
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    let mut found = collect_media_recursive(root);
    if found.is_empty() {
        return false;
    }
    sort_media(&mut found);

    found.retain(|p| !state.files.iter().any(|f| same_path(f, p)));
    if found.is_empty() {
        return false;
    }

    let start = state.files.len();
    state.nodes.push(PlaylistNode::Folder {
        name,
        path: root.to_path_buf(),
        files: found,
    });
    if state.dir.as_os_str().is_empty() {
        state.dir = root.to_path_buf();
    }
    state.rebuild_files();
    state.current = start.min(state.files.len().saturating_sub(1));
    true
}

/// 从扁平 index 删除一项，并同步从 nodes 里抠掉

/// 移除整个文件夹节点（node 下标，不是扁平文件下标）
pub fn remove_folder_node(state: &mut State, node_index: usize) -> bool {
    if node_index >= state.nodes.len() {
        return false;
    }
    let is_folder = matches!(state.nodes[node_index], PlaylistNode::Folder { .. });
    if is_folder {
        state.nodes.remove(node_index);
        state.rebuild_files();
        if state.files.is_empty() {
            state.current = 0;
        } else if state.current >= state.files.len() {
            state.current = state.files.len() - 1;
        }
        return true;
    }
    // 根级单文件：换算扁平 index 后删除
    let mut idx = 0usize;
    for (i, n) in state.nodes.iter().enumerate() {
        if i == node_index {
            break;
        }
        match n {
            PlaylistNode::File(_) => idx += 1,
            PlaylistNode::Folder { files, .. } => idx += files.len(),
        }
    }
    remove_at(state, idx)
}

pub fn remove_at(state: &mut State, index: usize) -> bool {
    if index >= state.files.len() {
        return false;
    }
    let target = state.files[index].clone();

    let mut new_nodes = Vec::new();
    for n in state.nodes.drain(..) {
        match n {
            PlaylistNode::File(p) => {
                if !same_path(&p, &target) {
                    new_nodes.push(PlaylistNode::File(p));
                }
            }
            PlaylistNode::Folder { name, path, mut files } => {
                files.retain(|p| !same_path(p, &target));
                if !files.is_empty() {
                    new_nodes.push(PlaylistNode::Folder { name, path, files });
                }
            }
        }
    }
    state.nodes = new_nodes;
    state.rebuild_files();
    if state.files.is_empty() {
        state.current = 0;
    } else {
        state.current = index.min(state.files.len() - 1);
    }
    true
}


/// 清空播放列表
pub fn clear_playlist(state: &mut State) {
    state.nodes.clear();
    state.files.clear();
    state.current = 0;
}

/// 根节点（文件夹 / 根级文件）之间换位：from_node → 插到 to_node 位置
pub fn reorder_nodes(state: &mut State, from: usize, to: usize) {
    let len = state.nodes.len();
    if from >= len || to > len || from == to {
        return;
    }
    let node = state.nodes.remove(from);
    let insert_at = if to > from { to - 1 } else { to };
    let insert_at = insert_at.min(state.nodes.len());
    state.nodes.insert(insert_at, node);
    let cur_path = state.files.get(state.current).cloned();
    state.rebuild_files();
    if let Some(p) = cur_path {
        if let Some(i) = state.files.iter().position(|f| same_path(f, &p)) {
            state.current = i;
        }
    }
}

/// 同一文件夹内视频换位（folder_node 下标 + 文件夹内 from/to）
pub fn reorder_in_folder(state: &mut State, folder_node: usize, from: usize, to: usize) {
    if folder_node >= state.nodes.len() {
        return;
    }
    let PlaylistNode::Folder { files, .. } = &mut state.nodes[folder_node] else {
        return;
    };
    let len = files.len();
    if from >= len || to > len || from == to {
        return;
    }
    let item = files.remove(from);
    let insert_at = if to > from { to - 1 } else { to };
    let insert_at = insert_at.min(files.len());
    files.insert(insert_at, item);
    let cur_path = state.files.get(state.current).cloned();
    state.rebuild_files();
    if let Some(p) = cur_path {
        if let Some(i) = state.files.iter().position(|f| same_path(f, &p)) {
            state.current = i;
        }
    }
}

/// 兼容旧接口：仅在「同一文件夹内」或「两个都是根级 File」时重排，保留文件夹结构
pub fn reorder(state: &mut State, from: usize, to: usize) {
    let len = state.files.len();
    // to == len 允许插到末尾
    if from >= len || to > len || from == to {
        return;
    }
    // 定位 from/to 属于哪个 node
    fn locate(nodes: &[PlaylistNode], flat: usize) -> Option<(usize, Option<usize>)> {
        // (node_index, Some(index_in_folder) | None if root File)
        let mut idx = 0usize;
        for (ni, n) in nodes.iter().enumerate() {
            match n {
                PlaylistNode::File(_) => {
                    if idx == flat {
                        return Some((ni, None));
                    }
                    idx += 1;
                }
                PlaylistNode::Folder { files, .. } => {
                    if flat >= idx && flat < idx + files.len() {
                        return Some((ni, Some(flat - idx)));
                    }
                    idx += files.len();
                }
            }
        }
        None
    }
    let Some((n_from, in_from)) = locate(&state.nodes, from) else {
        return;
    };
    let Some((n_to, in_to)) = locate(&state.nodes, to) else {
        return;
    };
    // 同一文件夹内
    if let (Some(a), Some(b)) = (in_from, in_to) {
        if n_from == n_to {
            reorder_in_folder(state, n_from, a, b);
            return;
        }
    }
    // 两个都是根级 File：用节点换位
    if in_from.is_none() && in_to.is_none() {
        reorder_nodes(state, n_from, n_to);
        return;
    }
    // 跨组：不拆文件夹，忽略（保持结构）
}


/// 拖入多个路径：文件挂根，文件夹挂【名】节点
pub fn add_paths_to_playlist(state: &mut State, paths: &[PathBuf]) -> bool {
    let mut any = false;
    for p in paths {
        if p.is_dir() {
            if add_folder_to_playlist(state, p) {
                any = true;
            }
        } else if p.is_file() {
            if add_file_to_playlist(state, p.clone()) {
                any = true;
            }
        }
    }
    any
}

pub fn pick_file_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("打开视频")
        .add_filter("视频", VIDEO_EXTS)
        .pick_file()
}

pub fn pick_folder_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("打开文件夹")
        .pick_folder()
}

/// %LOCALAPPDATA%/rust-videos/ui-prefs.json
pub fn ui_prefs_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("rust-videos").join("ui-prefs.json")
}

pub fn load_ui_prefs() -> serde_json::Value {
    let path = ui_prefs_path();
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

pub fn save_ui_prefs(v: &serde_json::Value) {
    let path = ui_prefs_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, v.to_string());
}
