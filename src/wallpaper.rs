use crate::cdp::Cdp;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        fs::{FileTypeExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

const KEY: &str = "window.__codexDragonWallpaperV1";
const CLEAN:&str = "(()=>{window.__codexDragonWallpaperV1?.dispose();document.getElementById('codex-dragon-linux-transition')?.remove();return {ok:true};})()";
const RENDERER: &str = include_str!("../renderer.js");
const TRANSITION: &str = include_str!("../transition.css");

pub fn accepted_app_url(url: &str) -> bool {
    if url == "app://-/index.html" {
        return true;
    }
    let Some(route) = url.strip_prefix("app://-/index.html?initialRoute=") else {
        return false;
    };
    if route.is_empty() || route.len() > 6144 || route.contains(['&', '#']) {
        return false;
    }
    let mut decoded = Vec::new();
    let mut chars = route.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let Some(a) = chars.next().and_then(|v| (v as char).to_digit(16)) else {
                return false;
            };
            let Some(b) = chars.next().and_then(|v| (v as char).to_digit(16)) else {
                return false;
            };
            decoded.push((a * 16 + b) as u8);
        } else {
            decoded.push(b);
        }
    }
    let Ok(route) = String::from_utf8(decoded) else {
        return false;
    };
    route.len() <= 2048
        && route.starts_with('/')
        && !route.starts_with("//")
        && !route.chars().any(|c| c.is_control())
}

fn guard(expression: &str, fixture: Option<&str>) -> String {
    let condition = if let Some(url) = fixture {
        format!("location.href==={}", json!(url))
    } else {
        "(()=>{try{const u=new URL(location.href),q=[...u.searchParams];return u.protocol==='app:'&&u.hostname==='-'&&u.pathname==='/index.html'&&!u.port&&!u.username&&!u.password&&!u.hash&&((!u.search&&!q.length)||(q.length===1&&q[0][0]==='initialRoute'&&q[0][1].length<=2048&&q[0][1].startsWith('/')&&!q[0][1].startsWith('//')&&!/[\\x00-\\x1f\\x7f]/.test(q[0][1])));}catch{return false;}})()".into()
    };
    format!("(()=>{{if(!({condition}))return {{guardRejected:true}};return ({expression});}})()")
}

pub fn eval(
    cdp: &mut Cdp,
    session: &str,
    expression: &str,
    fixture: Option<&str>,
    promise: bool,
) -> Result<Value, String> {
    let r=cdp.call("Runtime.evaluate",json!({"expression":guard(expression,fixture),"returnByValue":true,"awaitPromise":promise,"userGesture":false}),Some(session),18)?;
    if !r["exceptionDetails"].is_null() {
        return Err(if fixture.is_some() {
            format!(
                "Isolated renderer expression failed: {}",
                r["exceptionDetails"]["exception"]["description"]
            )
        } else {
            "Renderer expression failed".into()
        });
    }
    let v = r["result"]["value"].clone();
    if v["guardRejected"] == true {
        return Err("Application document guard rejected target".into());
    }
    Ok(v)
}

pub fn attach(
    cdp: &mut Cdp,
    session: &str,
    assets: &Path,
    fixture: Option<&str>,
) -> Result<Value, String> {
    let script = RENDERER.trim().trim_end_matches(';');
    let install=format!("(()=>{{const r=({script});if(r.ok){{const s=document.createElement('style');s.id='codex-dragon-linux-transition';s.textContent={};document.head.appendChild(s);const stage=document.getElementById('codex-dragon-stage-v1');for(const name of ['tear','ripple']){{const n=document.createElement('i');n.className='codex-dragon-'+name;stage.appendChild(n);}}}}return r;}})()",json!(TRANSITION));
    let r = eval(cdp, session, &install, fixture, false)?;
    if r["ok"] != true {
        return Err("Renderer installation rejected layout".into());
    }
    let result = (|| {
        let generation = fs::read_to_string("/proc/sys/kernel/random/uuid")
            .map_err(|e| e.to_string())?
            .trim()
            .replace('-', "");
        let configured = eval(
            cdp,
            session,
            &format!(
                "{KEY}.configure({{managed:true,documentGeneration:{}}})",
                json!(generation)
            ),
            fixture,
            false,
        )?;
        if configured["ok"] != true {
            return Err("Renderer configuration failed".into());
        }
        let doc = cdp.call("DOM.getDocument", json!({"depth":0}), Some(session), 3)?;
        let node = cdp.call(
            "DOM.querySelector",
            json!({"nodeId":doc["root"]["nodeId"],"selector":"#codex-dragon-file-v1"}),
            Some(session),
            3,
        )?;
        let paths = [
            "dragon-wallpaper-ambient-2k-144fps.mp4",
            "dragon-wallpaper-particles-1440p48.mp4",
        ]
        .iter()
        .map(|n| {
            assets
                .join(n)
                .canonicalize()
                .map(|p| p.to_string_lossy().into_owned())
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
        cdp.call(
            "DOM.setFileInputFiles",
            json!({"nodeId":node["nodeId"],"files":paths}),
            Some(session),
            3,
        )?;
        if crate::pressure::available().is_some_and(|v| v < 512 * 1024 * 1024) {
            eval(
                cdp,
                session,
                &format!("{KEY}.memoryPressure(true)"),
                fixture,
                false,
            )?;
        }
        let attached = eval(
            cdp,
            session,
            &format!("{KEY}.attach({{deferReveal:true}})"),
            fixture,
            true,
        )?;
        if attached["ok"] != true {
            return Err(format!("Media attach failed: {}", attached["code"]));
        }
        let reveal = eval(cdp, session, &format!("{KEY}.reveal()"), fixture, false)?;
        if reveal["ok"] != true {
            return Err("Wallpaper reveal failed".into());
        }
        let state = eval(cdp, session, &format!("{KEY}.status()"), fixture, false)?;
        Ok(
            json!({"generation":generation,"attached":true,"memoryPressure":state["memoryPressure"]}),
        )
    })();
    if result.is_err() {
        let _ = eval(cdp, session, CLEAN, fixture, false);
    }
    result
}

pub fn detach(cdp: &mut Cdp, session: &str, fixture: Option<&str>) -> Result<Value, String> {
    eval(cdp, session, CLEAN, fixture, false)
}

pub fn find_session(cdp: &mut Cdp, fixture: Option<&str>) -> Result<Option<String>, String> {
    let targets = cdp.call("Target.getTargets", json!({}), None, 3)?;
    let Some(targets) = targets["targetInfos"].as_array() else {
        return Ok(None);
    };
    let candidates: Vec<_> = targets
        .iter()
        .filter(|t| {
            t["type"] == "page"
                && t["url"]
                    .as_str()
                    .is_some_and(|u| fixture.map_or_else(|| accepted_app_url(u), |f| u == f))
        })
        .collect();
    if candidates.len() != 1 {
        return Ok(None);
    }
    let r = cdp.call(
        "Target.attachToTarget",
        json!({"targetId":candidates[0]["targetId"],"flatten":true}),
        None,
        3,
    )?;
    Ok(r["sessionId"].as_str().map(str::to_string))
}

fn control_path() -> Result<PathBuf, String> {
    let base = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is missing")?;
    Ok(PathBuf::from(base).join("codex-dragon/control.sock"))
}
pub fn control(command: &str) -> Result<Value, String> {
    control_at(&control_path()?, command)
}
pub fn control_at(path: &Path, command: &str) -> Result<Value, String> {
    let mut stream = UnixStream::connect(path).map_err(|_| {
        "No wallpaper host is active. Save work, quit Codex normally, then use Codex Dragon."
            .to_string()
    })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(4)))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(command.as_bytes())
        .map_err(|e| e.to_string())?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|e| e.to_string())?;
    let mut text = String::new();
    stream
        .take(8192)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

pub fn serve(mut cdp: Cdp, assets: PathBuf) -> Result<(), String> {
    let result = control_path().and_then(|path| serve_inner(&mut cdp, assets, &path, None, None));
    // Chromium's debug pipe is part of the owned browser lifetime. A setup
    // failure must not close it under an otherwise working application.
    if result.is_err() {
        while cdp.child.try_wait().ok().flatten().is_none() {
            thread::sleep(Duration::from_millis(500));
        }
    }
    result
}

pub fn serve_fixture(
    cdp: &mut Cdp,
    assets: &Path,
    path: &Path,
    fixture: &str,
    stop: &AtomicBool,
) -> Result<(), String> {
    serve_inner(cdp, assets.to_path_buf(), path, Some(fixture), Some(stop))
}

fn serve_inner(
    mut cdp: &mut Cdp,
    assets: PathBuf,
    path: &Path,
    fixture: Option<&str>,
    stop: Option<&AtomicBool>,
) -> Result<(), String> {
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    if let Ok(meta) = fs::symlink_metadata(&path) {
        if !meta.file_type().is_socket() {
            return Err("Unexpected control path; not replaced".into());
        }
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let mut session: Option<String> = None;
    let mut generation = String::new();
    let mut enabled = true;
    let mut status = json!({"phase":"waiting-for-workspace","enabled":true});
    let mut last_poll = Instant::now() - Duration::from_secs(5);
    let mut last_lease = Instant::now();
    let mut failures = 0;
    let mut recoveries = 0;
    let started = Instant::now();
    let mut pressure = crate::pressure::Policy::default();
    while !stop.is_some_and(|s| s.load(Ordering::Relaxed))
        && cdp.child.try_wait().map_err(|e| e.to_string())?.is_none()
    {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
            let mut buffer = Vec::new();
            let valid =
                (&mut stream).take(65).read_to_end(&mut buffer).is_ok() && buffer.len() <= 64;
            let command = if valid {
                std::str::from_utf8(&buffer).unwrap_or("").trim()
            } else {
                ""
            };
            match command {
                "off" => {
                    enabled = false;
                    let clean = session
                        .as_ref()
                        .map_or(true, |s| detach(&mut cdp, s, fixture).is_ok());
                    if clean {
                        if let Some(s) = &session {
                            let _ = cdp.call(
                                "Target.detachFromTarget",
                                json!({"sessionId":s}),
                                None,
                                3,
                            );
                        }
                        session = None;
                    }
                    status = json!({"phase":if clean{"disabled"}else{"cleanup-unconfirmed"},"enabled":false,"cleanupConfirmed":clean});
                }
                "on" => {
                    enabled = true;
                    failures = 0;
                    last_poll = Instant::now() - Duration::from_secs(5);
                }
                "status" => {}
                "test-reload" if fixture.is_some() => {
                    if let Some(s) = &session {
                        cdp.call("Page.reload", json!({"ignoreCache":true}), Some(s), 3)?;
                    }
                    status = json!({"phase":"reloading","enabled":true});
                }
                "test-media-error" if fixture.is_some() => {
                    if let Some(s) = &session {
                        eval(&mut cdp,s,"(()=>{const v=document.querySelector('#codex-dragon-stage-v1 video');v.src='blob:codex-dragon-self-test-invalid';v.load();return {ok:true};})()",fixture,false)?;
                    }
                    status = json!({"phase":"fault-injected","enabled":true});
                }
                _ => {
                    let _ = stream.write_all(b"{\"error\":\"unknown-command\"}");
                    continue;
                }
            }
            let _ = stream.write_all(status.to_string().as_bytes());
        }
        if enabled && last_poll.elapsed() >= Duration::from_secs(2) {
            last_poll = Instant::now();
            if session.is_none() && failures < 3 {
                match find_session(&mut cdp, fixture) {
                    Ok(Some(s)) => {
                        let ready=eval(&mut cdp,&s,"({ready:!!document.querySelector('#root main[data-app-shell-main-surface=\"default\"]')})",fixture,false).unwrap_or(Value::Null);
                        if ready["ready"] == true {
                            match attach(&mut cdp, &s, &assets, fixture) {
                                Ok(v) => {
                                    generation = v["generation"].as_str().unwrap_or("").into();
                                    session = Some(s);
                                    failures = 0;
                                    recoveries = 0;
                                    pressure = crate::pressure::Policy::default();
                                    pressure.paused = v["memoryPressure"] == true;
                                    last_lease = Instant::now();
                                    status = json!({"phase":"preparing","enabled":true});
                                }
                                Err(e) => {
                                    failures += 1;
                                    status = json!({"phase":"not-attached","enabled":true,"reason":e,"attempts":failures});
                                    let _ = cdp.call(
                                        "Target.detachFromTarget",
                                        json!({"sessionId":s}),
                                        None,
                                        3,
                                    );
                                }
                            }
                        } else {
                            let _ = cdp.call(
                                "Target.detachFromTarget",
                                json!({"sessionId":s}),
                                None,
                                3,
                            );
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        status = json!({"phase":"channel-unavailable","reason":e,"enabled":true});
                    }
                }
            }
            if let Some(s) = session.clone() {
                if let Some(available) = crate::pressure::available() {
                    if let Some(active) = pressure.update(available, started.elapsed()) {
                        let _ = eval(
                            &mut cdp,
                            &s,
                            &format!("{KEY}?.memoryPressure({active})"),
                            fixture,
                            false,
                        );
                    }
                }
                if last_lease.elapsed() >= Duration::from_secs(10) {
                    let _ = eval(
                        &mut cdp,
                        &s,
                        &format!("{KEY}?.renewLease({})", json!(generation)),
                        fixture,
                        false,
                    );
                    last_lease = Instant::now();
                }
                match eval(
                    &mut cdp,
                    &s,
                    &format!("{KEY}?.status()??{{missing:true}}"),
                    fixture,
                    false,
                ) {
                    Ok(v) if v["missing"] != true => {
                        status = json!({"phase":v["actualPhase"],"enabled":true,"playing":v["playing"],"firstFrameConfirmed":v["firstFrameConfirmed"],"wallpaperApplied":v["wallpaperApplied"],"clip":v["activeClip"],"layoutSupported":v["layoutSupported"],"layoutReason":v["layoutReason"],"videoErrorCode":v["videoErrorCode"],"health":v["health"],"memoryPressure":v["memoryPressure"]});
                        if v["recoveryStable"] == true {
                            recoveries = 0;
                        }
                        if v["actualPhase"] == "retired"
                            || (v["recoveryNeeded"] == true && recoveries >= 2)
                        {
                            let clean = detach(&mut cdp, &s, fixture).is_ok();
                            let _ = cdp.call(
                                "Target.detachFromTarget",
                                json!({"sessionId":s}),
                                None,
                                3,
                            );
                            session = None;
                            enabled = false;
                            status = json!({"phase":"disabled-after-renderer-error","enabled":false,"cleanupConfirmed":clean,"reason":v["code"]});
                        } else if v["recoveryNeeded"] == true {
                            if let Some(g) = v["mediaGeneration"].as_u64() {
                                let _ = eval(
                                    &mut cdp,
                                    &s,
                                    &format!("{KEY}.recover({g})"),
                                    fixture,
                                    false,
                                );
                                recoveries += 1;
                                status["phase"] = json!("recovering");
                            }
                        }
                    }
                    _ => {
                        let _ = detach(&mut cdp, &s, fixture);
                        let _ =
                            cdp.call("Target.detachFromTarget", json!({"sessionId":s}), None, 3);
                        session = None;
                        failures = 0;
                        status = json!({"phase":"waiting-for-workspace","enabled":true});
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    if let Some(s) = &session {
        let _ = detach(&mut cdp, s, fixture);
    }
    let _ = fs::remove_file(path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_web_pages_and_unrelated_app_pages() {
        for u in [
            "https://chatgpt.com/",
            "app://evil/index.html",
            "app://-/other",
            "app://-/index.html?initialRoute=%2F&x=1",
            "app://-/index.html?initialRoute=%2F%2Fevil",
            "app://-/index.html?initialRoute=%2F%00",
            "app://-/index.html?initialRoute=%ZZ",
            "file:///tmp/a.html",
        ] {
            assert!(!accepted_app_url(u));
        }
    }
    #[test]
    fn accepts_only_app_entry_and_initial_routes() {
        assert!(accepted_app_url("app://-/index.html"));
        assert!(accepted_app_url(
            "app://-/index.html?initialRoute=%2Fthreads%2Fx"
        ));
    }
}
