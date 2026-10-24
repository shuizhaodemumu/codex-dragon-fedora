use crate::{
    cdp::Cdp,
    wallpaper::{self, eval},
};
use base64::Engine;
use serde_json::json;
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

pub fn run(assets: &Path, output: &Path) -> Result<(), String> {
    fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let output = output.canonicalize().map_err(|e| e.to_string())?;
    let fixture = output.join("fixture.html");
    fs::write(&fixture,r##"<!doctype html><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none';style-src 'unsafe-inline';media-src blob:;script-src 'none'"><style>body{margin:0;color:#eee;font:18px sans-serif;background:#101518}#root{height:100vh;overflow:hidden}main{height:100vh;background:#101518}._WorkspaceContent_test_2{padding:60px;height:100%;box-sizing:border-box}h1{font-size:36px}button{padding:14px;border-radius:12px}._ComposerLayoutRoot_comp_2{position:absolute;bottom:50px;left:60px;right:60px;padding:22px;border-radius:20px;background:var(--composer-layout-surface-background,#202a31);backdrop-filter:var(--composer-layout-surface-backdrop-filter,none)}</style><div id="root"><main class="_MainContentSurface_test_2" data-app-shell-main-surface="default"><div class="_WorkspaceContent_test_2"><h1>Fedora 动态壁纸 · 本地验证页</h1><p>测试视频、透明输入区与清理恢复，不包含真实对话。</p><button id="test-button">测试按钮</button><div class="_ComposerLayoutRoot_comp_2" data-composer-surface-variant="default">输入区域测试 · 文字保持不透明</div></div></main></div>"##).map_err(|e|e.to_string())?;
    let url = format!("file://{}", fixture.display());
    let args = vec![
        "--headless=new".into(),
        "--no-first-run".into(),
        "--disable-background-networking".into(),
        "--autoplay-policy=no-user-gesture-required".into(),
        "--window-size=1440,900".into(),
        format!(
            "--user-data-dir={}",
            output.join("chrome-profile").display()
        ),
        url.clone(),
    ];
    let mut cdp = Cdp::spawn(Path::new("/usr/bin/google-chrome"), &args)?;
    let result = (|| {
        let end = Instant::now() + Duration::from_secs(15);
        let session = loop {
            if let Some(s) = wallpaper::find_session(&mut cdp, Some(&url))? {
                break s;
            }
            if Instant::now() > end {
                return Err("Fixture target did not appear".into());
            }
            thread::sleep(Duration::from_millis(200));
        };
        // Require a loaded fixture rather than treating target creation as document readiness.
        loop {
            if eval(
                &mut cdp,
                &session,
                "({ready:!!document.getElementById('test-button')})",
                Some(&url),
                false,
            )
            .ok()
            .is_some_and(|v| v["ready"] == true)
            {
                break;
            }
            if Instant::now() > end {
                return Err("Fixture document did not become ready".into());
            }
            thread::sleep(Duration::from_millis(200));
        }
        let attached = wallpaper::attach(&mut cdp, &session, assets, Some(&url))?;
        let end = Instant::now() + Duration::from_secs(15);
        let first = loop {
            let status = eval(
                &mut cdp,
                &session,
                "window.__codexDragonWallpaperV1.status()",
                Some(&url),
                false,
            )?;
            if status["playing"] == true
                && status["firstFrameConfirmed"] == true
                && status["wallpaperApplied"] == true
            {
                break status;
            }
            if Instant::now() > end {
                return Err(format!("No confirmed playback: {status}"));
            }
            thread::sleep(Duration::from_millis(250));
        };
        thread::sleep(Duration::from_millis(700));
        let second = eval(
            &mut cdp,
            &session,
            "window.__codexDragonWallpaperV1.status()",
            Some(&url),
            false,
        )?;
        if second["mediaTimeMs"].as_u64() <= first["mediaTimeMs"].as_u64() {
            return Err("Video clock did not advance".into());
        }
        let ui=eval(&mut cdp,&session,"(()=>{const b=document.getElementById('test-button');b.onclick=()=>b.dataset.clicked='yes';const r=b.getBoundingClientRect(),hit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);hit.click();const c=document.querySelector('[data-composer-surface-variant]');return{clickWorked:b.dataset.clicked==='yes',composerGlass:getComputedStyle(c).getPropertyValue('--composer-layout-surface-background').trim().length>0,transparentMain:getComputedStyle(document.querySelector('main')).backgroundColor==='rgba(0, 0, 0, 0)',transitionPresent:!!document.querySelector('.codex-dragon-ripple')};})()",Some(&url),false)?;
        if ui["clickWorked"] != true
            || ui["composerGlass"] != true
            || ui["transparentMain"] != true
            || ui["transitionPresent"] != true
        {
            return Err(format!("UI verification failed: {ui}"));
        }
        let image = cdp.call(
            "Page.captureScreenshot",
            json!({"format":"png","captureBeyondViewport":false}),
            Some(&session),
            5,
        )?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(image["data"].as_str().ok_or("Screenshot unavailable")?)
            .map_err(|e| e.to_string())?;
        fs::write(output.join("wallpaper-preview.png"), bytes).map_err(|e| e.to_string())?;
        // Navigation within the SPA preserves the renderer and playback.
        let moved=eval(&mut cdp,&session,"(()=>{const m=document.querySelector('main');m.parentElement.appendChild(m);return window.__codexDragonWallpaperV1.status();})()",Some(&url),false)?;
        if moved["layoutSupported"] != true {
            return Err("Layout move invalidated the wallpaper".into());
        }
        wallpaper::detach(&mut cdp, &session, Some(&url))?;
        let clean=eval(&mut cdp,&session,"({noApi:!window.__codexDragonWallpaperV1,noNodes:!document.querySelector('#codex-dragon-stage-v1,#codex-dragon-file-v1,#codex-dragon-style-v1,#codex-dragon-linux-transition'),noMarker:!document.documentElement.hasAttribute('data-codex-dragon-background'),backgroundRestored:getComputedStyle(document.querySelector('main')).backgroundColor==='rgb(16, 21, 24)'})",Some(&url),false)?;
        if clean
            .as_object()
            .is_none_or(|m| m.values().any(|v| v != true))
        {
            return Err(format!("Cleanup failed: {clean}"));
        }
        // Re-enable after disposal; exercise the second media file independently.
        let _ = wallpaper::attach(&mut cdp, &session, assets, Some(&url))?;
        let particle=eval(&mut cdp,&session,"(async()=>{window.__codexDragonWallpaperV1.pause();const input=document.getElementById('codex-dragon-file-v1'),v=document.createElement('video');v.muted=true;v.playsInline=true;v.style.cssText='position:fixed;width:1px;height:1px;bottom:0;right:0;pointer-events:none';document.body.appendChild(v);const url=URL.createObjectURL(input.files[1]);try{v.src=url;await v.play();await new Promise(r=>setTimeout(r,700));return{ready:v.readyState>=2,time:v.currentTime,width:v.videoWidth};}finally{v.pause();v.remove();URL.revokeObjectURL(url);}})()",Some(&url),true)?;
        if particle["ready"] != true || particle["time"].as_f64().unwrap_or(0.) <= 0. {
            return Err(format!("Particle video did not play: {particle}"));
        }
        wallpaper::detach(&mut cdp, &session, Some(&url))?;
        let _ = cdp.call(
            "Target.detachFromTarget",
            json!({"sessionId":session}),
            None,
            3,
        );
        let stop = AtomicBool::new(false);
        let socket = output.join("host-control.sock");
        let host_checks = thread::scope(|scope| {
            let worker =
                scope.spawn(|| wallpaper::serve_fixture(&mut cdp, assets, &socket, &url, &stop));
            let checks: Result<serde_json::Value, String> = (|| {
                let poll = || -> Result<serde_json::Value, String> {
                    let end = Instant::now() + Duration::from_secs(35);
                    loop {
                        if let Ok(s) = wallpaper::control_at(&socket, "status") {
                            if s["playing"] == true
                                && s["firstFrameConfirmed"] == true
                                && s["wallpaperApplied"] == true
                            {
                                return Ok(s);
                            }
                        }
                        if Instant::now() > end {
                            return Err("Host did not confirm live wallpaper playback".into());
                        }
                        thread::sleep(Duration::from_millis(250));
                    }
                };
                let first = poll()?;
                let off = wallpaper::control_at(&socket, "off")?;
                if off["cleanupConfirmed"] != true || off["enabled"] != false {
                    return Err("Host off did not confirm cleanup".into());
                }
                wallpaper::control_at(&socket, "on")?;
                let enabled = poll()?;
                wallpaper::control_at(&socket, "test-media-error")?;
                let recovered = poll()?;
                wallpaper::control_at(&socket, "test-reload")?;
                let reload = poll()?;
                let off_again = wallpaper::control_at(&socket, "off")?;
                if off_again["cleanupConfirmed"] != true {
                    return Err("Post-reload cleanup failed".into());
                }
                Ok(
                    json!({"first":first,"off":off,"reenabled":enabled,"recoveredFromMediaError":recovered,"reload":reload,"finalOff":off_again}),
                )
            })();
            stop.store(true, Ordering::Relaxed);
            let server = worker
                .join()
                .map_err(|_| "Host test thread failed".to_string())?;
            server?;
            checks
        })?;
        let report = json!({"scope":"Isolated Chrome fixture; not the live Codex account","passed":true,"attach":attached,"firstPlayback":first,"laterPlayback":second,"ui":ui,"cleanup":clean,"particleDecode":particle,"reenable":true,"hostControls":host_checks});
        fs::write(
            output.join("results.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        println!(
            "Wallpaper self-test passed; report: {}",
            output.join("results.json").display()
        );
        Ok(())
    })();
    let _ = cdp.call("Browser.close", json!({}), None, 3);
    let end = Instant::now() + Duration::from_secs(5);
    while cdp.child.try_wait().ok().flatten().is_none() && Instant::now() < end {
        thread::sleep(Duration::from_millis(50));
    }
    if cdp.child.try_wait().ok().flatten().is_none() {
        let _ = cdp.child.kill();
        let _ = cdp.child.wait();
    }
    result
}
