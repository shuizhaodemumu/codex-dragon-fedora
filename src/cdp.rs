//! NUL-framed CDP on an inherited Unix socket; no TCP listener.
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub struct Cdp {
    pub child: Child,
    stream: UnixStream,
    buffer: Vec<u8>,
    next_id: u64,
}

impl Cdp {
    pub fn spawn(exe: &Path, args: &[String]) -> Result<Self, String> {
        let (stream, child_socket) = UnixStream::pair().map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .map_err(|e| e.to_string())?;
        let raw = unsafe { libc::fcntl(child_socket.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
        if raw < 0 {
            return Err("Cannot allocate private CDP descriptor".into());
        }
        let inherited = unsafe { OwnedFd::from_raw_fd(raw) };
        let mut command = Command::new(exe);
        command
            .arg("--remote-debugging-pipe")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(inherited.as_raw_fd(), 3) < 0
                    || libc::dup2(inherited.as_raw_fd(), 4) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().map_err(|e| e.to_string())?;
        drop(command);
        drop(child_socket);
        Ok(Self {
            child,
            stream,
            buffer: Vec::new(),
            next_id: 0,
        })
    }

    pub fn call(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
        seconds: u64,
    ) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let mut request = json!({"id":id,"method":method,"params":params});
        if let Some(session) = session {
            request["sessionId"] = json!(session);
        }
        let mut bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        bytes.push(0);
        self.stream
            .write_all(&bytes)
            .map_err(|_| "Private CDP write failed".to_string())?;
        let end = Instant::now() + Duration::from_secs(seconds);
        while Instant::now() < end {
            if let Some(pos) = self.buffer.iter().position(|b| *b == 0) {
                let frame = self.buffer.drain(..=pos).collect::<Vec<_>>();
                let response: Value =
                    serde_json::from_slice(&frame[..pos]).map_err(|_| "Invalid CDP frame")?;
                if response["id"].as_u64() != Some(id) {
                    continue;
                }
                if let Some(expected) = session {
                    if response["sessionId"].as_str() != Some(expected) {
                        return Err("CDP session mismatch".into());
                    }
                }
                if !response["error"].is_null() {
                    return Err(format!(
                        "CDP rejected {method} (code {})",
                        response["error"]["code"]
                    ));
                }
                return Ok(response["result"].clone());
            }
            if self.buffer.len() > 8 * 1024 * 1024 {
                return Err("Oversized CDP response".into());
            }
            let mut chunk = [0u8; 16384];
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err("Private CDP channel closed".into()),
                Ok(n) => self.buffer.extend_from_slice(&chunk[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => return Err("Private CDP read failed".into()),
            }
        }
        Err(format!("CDP request timed out: {method}"))
    }
}
