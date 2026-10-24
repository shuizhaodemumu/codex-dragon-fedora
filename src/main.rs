use std::{
    env,
    fs::{self, File, OpenOptions},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
mod cdp;
mod pressure;
mod selftest;
mod wallpaper;

unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}

#[derive(Default)]
struct Options {
    preview: bool,
    check: bool,
    skip: bool,
    force: bool,
    wallpaper: bool,
    no_wallpaper: bool,
    control: Option<String>,
    self_test: Option<PathBuf>,
    app: Option<PathBuf>,
    video: Option<PathBuf>,
    args: Vec<String>,
}

fn options(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut result = Options::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--preview" => result.preview = true,
            "--check" => result.check = true,
            "--skip-animation" => result.skip = true,
            "--force-animation" => result.force = true,
            "--wallpaper" => result.wallpaper = true,
            "--no-wallpaper" => result.no_wallpaper = true,
            "--wallpaper-status" => result.control = Some("status".into()),
            "--wallpaper-off" => result.control = Some("off".into()),
            "--wallpaper-on" => result.control = Some("on".into()),
            "--wallpaper-self-test" => {
                result.self_test = Some(
                    args.next()
                        .ok_or("--wallpaper-self-test requires an output directory")?
                        .into(),
                )
            }
            "--app" => result.app = Some(args.next().ok_or("--app requires a path")?.into()),
            "--video" => result.video = Some(args.next().ok_or("--video requires a path")?.into()),
            "--" => {
                result.args.extend(args);
                break;
            }
            "--help" | "-h" => return Err("help".into()),
            _ if arg.starts_with('-') => return Err(format!("Unknown option: {arg}")),
            _ => result.args.push(arg),
        }
    }
    Ok(result)
}

fn available(name: &str) -> bool {
    env::var_os("PATH")
        .map(|p| env::split_paths(&p).any(|d| d.join(name).is_file()))
        .unwrap_or(false)
}

fn video_path(opts: &Options) -> PathBuf {
    if let Some(path) = &opts.video {
        return path.clone();
    }
    let exe = env::current_exe().unwrap_or_default();
    let adjacent = exe
        .parent()
        .unwrap_or(Path::new("."))
        .join("assets/startup.mp4");
    if adjacent.is_file() {
        return adjacent;
    }
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".local/share")
        });
    data.join("codex-dragon/assets/startup.mp4")
}

fn lock() -> Result<File, String> {
    let runtime = env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is missing")?;
    let path = PathBuf::from(runtime).join("codex-dragon-launch.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(0o400000)
        .open(path)
        .map_err(|e| e.to_string())?;
    if unsafe { flock(file.as_raw_fd(), 2 | 4) } != 0 {
        return Err("Another animation launcher is active".into());
    }
    Ok(file)
}

fn already_running() -> bool {
    let Ok(entries) = fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        let Ok(exe) = fs::read_link(path.join("exe")) else {
            return false;
        };
        if exe != Path::new("/usr/lib/chatgpt/ChatGPT") {
            return false;
        }
        let Ok(args) = fs::read(path.join("cmdline")) else {
            return false;
        };
        !args.split(|b| *b == 0).any(|a| a.starts_with(b"--type="))
    })
}

fn animate(video: &Path) -> Result<(), String> {
    if !video.is_file() {
        return Err(format!("Animation file missing: {}", video.display()));
    }
    let mut command = Command::new("ffplay");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostats",
            "-autoexit",
            "-an",
            "-noborder",
            "-fs",
            "-window_title",
            "Codex Dragon — ESC to skip",
            "-x",
            "1280",
            "-y",
            "720",
        ])
        .arg(video)
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    // SDL's automatic X11 choice can yield an unmanaged fullscreen surface
    // under Xwayland-satellite. Prefer the session's native Wayland backend.
    if env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland")
        && env::var_os("WAYLAND_DISPLAY").is_some()
        && env::var_os("SDL_VIDEODRIVER").is_none()
    {
        command.env("SDL_VIDEODRIVER", "wayland");
    }
    let mut player = command.spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match player.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!("Player exited: {status}"))
                }
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = player.kill();
                let _ = player.wait();
                return Err("Animation timed out; continuing to Codex".into());
            }
        }
    }
}

fn main() {
    let opts = match options(env::args().skip(1)) {
        Ok(o) => o,
        Err(e) => {
            if e != "help" {
                eprintln!("{e}");
            }
            println!("codex-dragon [--preview|--check|--skip-animation] [--force-animation] [--video PATH] [--app PATH] [-- URI ...]\nWallpaper is enabled on cold launches when its assets are installed.\n--no-wallpaper | --wallpaper-status | --wallpaper-off | --wallpaper-on\n--wallpaper-self-test OUTPUT_DIR tests an isolated browser fixture.\nESC/q skips animation. Animation failure still opens Codex. No app files are modified.");
            std::process::exit(if e == "help" { 0 } else { 2 });
        }
    };
    if let Some(command) = &opts.control {
        match wallpaper::control(command) {
            Ok(v) => println!("{v}"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let video = video_path(&opts);
    let assets = video.parent().unwrap_or(Path::new(".")).to_path_buf();
    if let Some(output) = &opts.self_test {
        if let Err(e) = selftest::run(&assets, output) {
            eprintln!("Wallpaper self-test failed: {e}");
            std::process::exit(1);
        }
        return;
    }
    if opts.check {
        let app = opts.app.as_deref().unwrap_or(Path::new("/usr/bin/chatgpt"));
        println!(
            "video={} exists={}\nffplay={}\napp={} exists={}\nalready_running={}\nsession={}",
            video.display(),
            video.is_file(),
            available("ffplay"),
            app.display(),
            app.is_file(),
            already_running(),
            env::var("XDG_SESSION_TYPE").unwrap_or_default()
        );
        if !video.is_file() || !available("ffplay") || !app.is_file() {
            std::process::exit(1);
        }
        return;
    }
    // Keep the descriptor open throughout playback; kernel releases it even after a crash.
    let _guard = match lock() {
        Ok(file) => Some(file),
        Err(e) => {
            eprintln!("{e}");
            if opts.preview {
                std::process::exit(1);
            }
            None
        }
    };
    let running = already_running();
    let with_wallpaper = !opts.preview
        && !opts.no_wallpaper
        && _guard.is_some()
        && !running
        && (opts.app.is_none() || opts.wallpaper)
        && assets
            .join("dragon-wallpaper-ambient-2k-144fps.mp4")
            .is_file()
        && assets
            .join("dragon-wallpaper-particles-1440p48.mp4")
            .is_file();
    let owned = if with_wallpaper {
        let app = opts
            .app
            .as_deref()
            .unwrap_or(Path::new("/usr/lib/chatgpt/ChatGPT"));
        match cdp::Cdp::spawn(app, &opts.args) {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("Wallpaper host could not start: {e}");
                None
            }
        }
    } else {
        None
    };
    if !opts.skip && _guard.is_some() && (opts.preview || opts.force || !running) {
        if let Err(e) = animate(&video) {
            eprintln!("{e}");
            if opts.preview {
                std::process::exit(1);
            }
        }
    }
    drop(_guard);
    if let Some(client) = owned {
        if let Err(e) = wallpaper::serve(client, assets) {
            eprintln!("Wallpaper host: {e}");
        }
        return;
    }
    if !opts.preview {
        let app = opts.app.as_deref().unwrap_or(Path::new("/usr/bin/chatgpt"));
        match Command::new(app)
            .args(&opts.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Could not launch {}: {e}", app.display());
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Options, String> {
        options(args.iter().map(|a| a.to_string()))
    }
    #[test]
    fn preserves_uri_and_arguments() {
        let opts = parse(&[
            "--skip-animation",
            "--",
            "codex://threads/example",
            "--literal",
        ])
        .unwrap();
        assert!(opts.skip);
        assert_eq!(opts.args, ["codex://threads/example", "--literal"]);
    }
    #[test]
    fn rejects_missing_value_and_unknown_flag() {
        assert!(parse(&["--app"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
    }
    #[test]
    fn explicit_preview_and_media() {
        let opts = parse(&["--preview", "--video", "/tmp/custom.mp4"]).unwrap();
        assert!(opts.preview);
        assert_eq!(video_path(&opts), Path::new("/tmp/custom.mp4"));
    }
}
