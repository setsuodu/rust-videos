//! Call system PATH ffmpeg for video edits (ImageStudio-compatible, not bundled)
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(windows)]
fn path_str(p: &Path) -> String {
    let s = p.to_string_lossy();
    s.replace('/', "\\")
}

#[cfg(not(windows))]
fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// 缓存检测结果，避免每次打开编辑面板都 spawn ffmpeg（首次仍可能稍慢）
pub fn available() -> bool {
    use std::sync::OnceLock;
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        cmd.status().map(|s| s.success()).unwrap_or(false)
    })
}

fn run(args: &[String]) -> Result<(), String> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("无法启动 ffmpeg：{e}（请确认已安装并加入 PATH）"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: String = err.chars().rev().take(400).collect::<String>().chars().rev().collect();
        return Err(format!("ffmpeg 失败：{tail}"));
    }
    Ok(())
}

fn sibling(src: &Path, suffix: &str, force_ext: Option<&str>) -> PathBuf {
    let parent = src.parent().unwrap_or_else(|| Path::new("."));
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    let ext = force_ext
        .or_else(|| src.extension().and_then(|e| e.to_str()))
        .unwrap_or("mp4");
    let mut path = parent.join(format!("{stem}_{suffix}.{ext}"));
    if path.exists() {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        path = parent.join(format!("{stem}_{suffix}_{t}.{ext}"));
    }
    path
}

fn require() -> Result<(), String> {
    if available() {
        Ok(())
    } else {
        Err("未检测到 ffmpeg。请安装并加入 PATH 后重启本软件。下载：https://ffmpeg.org/download.html".into())
    }
}

/// Time trim (-c copy)
pub fn trim(src: &Path, start: f64, end: Option<f64>, duration: Option<f64>) -> Result<PathBuf, String> {
    require()?;
    let start = start.max(0.0);
    let suffix = if let Some(e) = end {
        format!("trim_{:.0}-{:.0}s", start, e)
    } else if let Some(d) = duration {
        format!("trim_{:.0}s_len{:.0}", start, d)
    } else {
        format!("trim_from_{:.0}s", start)
    };
    let out = sibling(src, &suffix, None);
    let mut args = vec!["-y".into()];
    if start > 0.0 {
        args.push("-ss".into());
        args.push(format!("{start:.3}"));
    }
    args.push("-i".into());
    args.push(path_str(src));
    if let Some(d) = duration {
        if d > 0.0 {
            args.push("-t".into());
            args.push(format!("{d:.3}"));
        }
    } else if let Some(e) = end {
        if e > start {
            args.push("-to".into());
            args.push(format!("{e:.3}"));
        }
    }
    args.extend([
        "-c".into(),
        "copy".into(),
        "-avoid_negative_ts".into(),
        "make_zero".into(),
        path_str(&out),
    ]);
    run(&args)?;
    Ok(out)
}

/// Bitrate kbps -> mp4
pub fn bitrate(src: &Path, kbps: i64) -> Result<PathBuf, String> {
    require()?;
    let kbps = kbps.clamp(100, 50_000);
    let out = sibling(src, &format!("{kbps}k"), Some("mp4"));
    let args = vec![
        "-y".into(),
        "-i".into(),
        path_str(src),
        "-b:v".into(),
        format!("{kbps}k"),
        "-c:v".into(),
        "libx264".into(),
        "-c:a".into(),
        "copy".into(),
        path_str(&out),
    ];
    run(&args)?;
    Ok(out)
}

/// Horizontal split left | right | both
pub fn split_h(src: &Path, side: &str) -> Result<Vec<PathBuf>, String> {
    require()?;
    let side = side.to_ascii_lowercase();
    let jobs: Vec<(&str, &str)> = match side.as_str() {
        "right" => vec![("right", "crop=iw/2:ih:iw/2:0")],
        "both" => vec![
            ("left", "crop=iw/2:ih:0:0"),
            ("right", "crop=iw/2:ih:iw/2:0"),
        ],
        _ => vec![("left", "crop=iw/2:ih:0:0")],
    };
    let mut outs = Vec::new();
    for (suf, vf) in jobs {
        let out = sibling(src, suf, None);
        let args = vec![
            "-y".into(),
            "-i".into(),
            path_str(src),
            "-vf".into(),
            vf.into(),
            "-c:a".into(),
            "copy".into(),
            path_str(&out),
        ];
        run(&args)?;
        outs.push(out);
    }
    Ok(outs)
}

/// Change aspect ratio (ImageStudio ChangeAspect)
/// ratio: "16:9" / "9:16" / "1:1" ...
/// mode: stretch | crop | pad
pub fn change_aspect(src: &Path, ratio: &str, mode: &str) -> Result<PathBuf, String> {
    require()?;
    let ratio = ratio.trim().replace('：', ":");
    let mode = mode.trim().to_ascii_lowercase();
    let mode = if mode == "scale" {
        "stretch".to_string()
    } else {
        mode
    };
    if mode != "stretch" && mode != "crop" && mode != "pad" {
        return Err("mode 仅支持 stretch / crop / pad".into());
    }
    let parts: Vec<&str> = ratio
        .split(|c| c == ':' || c == '/' || c == 'x' || c == 'X' || c == '*')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() != 2 {
        return Err(format!("无效比例: {ratio}，示例 16:9 / 4:3 / 9:16"));
    }
    let rw: f64 = parts[0]
        .parse()
        .map_err(|_| format!("无效比例: {ratio}"))?;
    let rh: f64 = parts[1]
        .parse()
        .map_err(|_| format!("无效比例: {ratio}"))?;
    if rw <= 0.0 || rh <= 0.0 {
        return Err(format!("无效比例: {ratio}"));
    }
    let tar = rw / rh;
    let inv = rh / rw;
    let label = ratio.replace(':', "x");
    let out = sibling(src, &format!("{label}_{mode}"), Some("mp4"));

    let vf = match mode.as_str() {
        "crop" => format!(
            "scale=iw*sar:ih,setsar=1,\
crop=min(iw\\,ih*{tar}):min(ih\\,iw*{inv}):(iw-ow)/2:(ih-oh)/2,\
scale=trunc(iw/2)*2:trunc(ih/2)*2,setsar=1"
        ),
        "pad" => format!(
            "scale=iw*sar:ih,setsar=1,\
pad=max(iw\\,ih*{tar}):max(ih\\,iw*{inv}):(ow-iw)/2:(oh-ih)/2:black,\
scale=trunc(iw/2)*2:trunc(ih/2)*2,setsar=1"
        ),
        _ => format!(
            "scale=iw*sar:ih,setsar=1,\
scale=trunc(if(lte({tar}\\,1)\\,max(iw\\,ih)*{tar}\\,max(iw\\,ih))/2)*2:\
trunc(if(lte({tar}\\,1)\\,max(iw\\,ih)\\,max(iw\\,ih)*{inv})/2)*2,\
setsar=1"
        ),
    };

    let args = vec![
        "-y".into(),
        "-i".into(),
        path_str(src),
        "-vf".into(),
        vf,
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "fast".into(),
        "-crf".into(),
        "18".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-movflags".into(),
        "+faststart".into(),
        path_str(&out),
    ];
    run(&args)?;
    Ok(out)
}
