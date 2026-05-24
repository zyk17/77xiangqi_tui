use std::io::{BufRead, BufReader, Write};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError};

use super::engine_core::UciUcciEngine;
use super::engine_path::has_non_ascii;
use super::types::EngineStdoutPoll;
use crate::runtime_log;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub(crate) struct EngineRt {
    pub(crate) _child: Child,
    pub(crate) stdin: Mutex<ChildStdin>,
    pub(crate) lines: Receiver<String>,
    pub(crate) _stdout_reader: thread::JoinHandle<()>,
    pub(crate) _stderr_reader: Option<thread::JoinHandle<()>>,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn unix_is_executable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| (m.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn try_make_unix_executable(path: &Path) -> Result<bool, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let mut perms = meta.permissions();
    let mode = perms.mode();
    if (mode & 0o111) != 0 {
        return Ok(true);
    }
    perms.set_mode(mode | 0o111);
    std::fs::set_permissions(path, perms).map_err(|e| e.to_string())?;
    Ok(unix_is_executable(path))
}

#[cfg(target_os = "macos")]
fn try_clear_macos_quarantine(path: &Path) {
    let out = Command::new("xattr")
        .arg("-d")
        .arg("com.apple.quarantine")
        .arg(path.as_os_str())
        .output();
    match out {
        Ok(o) if o.status.success() => {
            runtime_log::debug(format!(
                "[engine_spawn] auto_clear_quarantine_ok path={}",
                path.display()
            ));
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            runtime_log::debug(format!(
                "[engine_spawn] auto_clear_quarantine_skip path={} status={} stderr={}",
                path.display(),
                o.status,
                stderr.trim()
            ));
        }
        Err(e) => {
            runtime_log::debug(format!(
                "[engine_spawn] auto_clear_quarantine_unavailable path={} err={e}",
                path.display()
            ));
        }
    }
}
impl UciUcciEngine {
    pub(crate) fn terminate_locked(&self) {
        let mut g = self.rt.lock().unwrap();
        if let Some(mut rt) = g.take() {
            let _ = rt._child.kill();
            let _ = rt._child.wait();
            let _ = rt._stdout_reader.join();
            if let Some(h) = rt._stderr_reader.take() {
                let _ = h.join();
            }
        }
    }

    fn child_exit_status_string(&self) -> Option<String> {
        let mut g = self.rt.lock().ok()?;
        let rt = g.as_mut()?;
        match rt._child.try_wait() {
            Ok(Some(status)) => Some(status.to_string()),
            Ok(None) => None,
            Err(e) => Some(format!("try_wait_err={e}")),
        }
    }

    pub(crate) fn spawn_process(&mut self) -> Result<(), String> {
        self.terminate_locked();
        let path_owned: String = self
            .engine_path
            .as_ref()
            .ok_or("未设置引擎路径：请在设置中选择中国象棋引擎可执行文件（需支持 UCI 或 UCCI）")?
            .clone();
        let path_obj = Path::new(&path_owned);
        let path_exists = path_obj.exists();
        let path_is_file = path_obj.is_file();
        let parent = path_obj.parent().unwrap_or(Path::new("."));
        runtime_log::debug(format!(
            "[engine_spawn] path={} non_ascii={} exists={} is_file={} parent={}",
            path_owned,
            has_non_ascii(&path_owned),
            path_exists,
            path_is_file,
            parent.display()
        ));
        #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
        if !path_is_file {
            runtime_log::warn(format!(
                "[engine_spawn] abort reason=engine_path_not_file path={}",
                path_owned
            ));
            return Err("引擎文件不存在，请检查路径或重新选择".into());
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            #[cfg(target_os = "macos")]
            {
                // 静默 best-effort：常见下载引擎会带 quarantine，先尝试移除。
                try_clear_macos_quarantine(path_obj);
            }
            let mut executable = unix_is_executable(path_obj);
            if !executable {
                match try_make_unix_executable(path_obj) {
                    Ok(true) => {
                        runtime_log::debug(format!(
                            "[engine_spawn] auto_chmod_ok path={}",
                            path_owned
                        ));
                        executable = true;
                    }
                    Ok(false) => {}
                    Err(e) => {
                        runtime_log::warn(format!(
                            "[engine_spawn] auto_chmod_failed path={} err={e}",
                            path_owned
                        ));
                    }
                }
            }
            if !executable {
                runtime_log::warn(format!(
                    "[engine_spawn] abort reason=engine_not_executable path={}",
                    path_owned
                ));
                return Err("引擎文件不可执行（缺少 +x 权限），请执行 chmod +x 后重试".into());
            }
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        if !path_is_file {
            runtime_log::warn(format!(
                "[engine_spawn] mobile_engine_path_not_file path={}",
                path_owned
            ));
        }
        let engine_dir = parent;
        let path_has_non_ascii = has_non_ascii(&path_owned);
        let mut cmd = Command::new(&path_owned);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if !path_has_non_ascii {
            cmd.current_dir(engine_dir);
        } else {
            runtime_log::debug(format!(
                "[engine_spawn] skip_current_dir reason=non_ascii_path cwd_fallback=process_default path={}",
                path_owned
            ));
        }
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn().map_err(|e| {
            runtime_log::error(format!(
                "[engine_spawn] spawn_err path={} cwd={} err={}",
                path_owned,
                engine_dir.display(),
                e
            ));
            e.to_string()
        })?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stderr: Option<ChildStderr> = child.stderr.take();
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let (tx, rx) = crossbeam_channel::unbounded::<String>();
        let stdout_reader = thread::spawn(move || {
            let mut br = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match br.read_line(&mut line) {
                    Ok(0) => {
                        runtime_log::debug("[engine_io] stdout_eof");
                        break;
                    }
                    Ok(_) => {
                        let _ = tx.send(line.clone());
                    }
                    Err(e) => {
                        runtime_log::debug(format!("[engine_io] stdout_read_err err={e}"));
                        break;
                    }
                }
            }
        });
        let stderr_reader = stderr.map(|stderr| {
            thread::spawn(move || {
                let mut br = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match br.read_line(&mut line) {
                        Ok(0) => {
                            runtime_log::debug("[engine_io] stderr_eof");
                            break;
                        }
                        Ok(_) => {
                            let msg = line.trim();
                            if !msg.is_empty() {
                                runtime_log::debug(format!("[engine_stderr] {msg}"));
                            }
                        }
                        Err(e) => {
                            runtime_log::debug(format!("[engine_io] stderr_read_err err={e}"));
                            break;
                        }
                    }
                }
            })
        });
        *self.rt.lock().unwrap() = Some(EngineRt {
            _child: child,
            stdin: Mutex::new(stdin),
            lines: rx,
            _stdout_reader: stdout_reader,
            _stderr_reader: stderr_reader,
        });
        self.handshake().inspect_err(|_e| {
            self.terminate_locked();
        })
    }

    pub(crate) fn send_cmd(&self, cmd: &str) -> Result<(), String> {
        let mut need_cleanup = false;
        let result = {
            let mut rt = self.rt.lock().map_err(|e| e.to_string())?;
            let Some(r) = rt.as_mut() else {
                return Err("engine not running".into());
            };
            match r._child.try_wait() {
                Ok(Some(status)) => {
                    need_cleanup = true;
                    Err(format!("engine exited before send, status={status}"))
                }
                Ok(None) => {
                    let mut stdin = r.stdin.lock().map_err(|e| e.to_string())?;
                    writeln!(stdin, "{cmd}").map_err(|e| e.to_string())?;
                    stdin.flush().map_err(|e| e.to_string())
                }
                Err(e) => Err(format!("engine try_wait failed: {e}")),
            }
        };
        if need_cleanup {
            self.terminate_locked();
        }
        if let Err(ref e) = result {
            runtime_log::warn(format!("[engine_io] send_cmd_failed cmd={cmd} err={e}"));
        }
        result
    }

    pub(crate) fn poll_line(&self, timeout: Duration) -> EngineStdoutPoll {
        let lines = {
            let rt = match self.rt.lock() {
                Ok(g) => g,
                Err(_) => {
                    return EngineStdoutPoll::Disconnected {
                        child_status: "runtime_lock_poisoned".to_string(),
                    };
                }
            };
            let Some(r) = rt.as_ref() else {
                return EngineStdoutPoll::Disconnected {
                    child_status: "runtime_not_running".to_string(),
                };
            };
            r.lines.clone()
        };
        match lines.recv_timeout(timeout) {
            Ok(s) => EngineStdoutPoll::Line(s),
            Err(RecvTimeoutError::Timeout) => EngineStdoutPoll::Tick,
            Err(RecvTimeoutError::Disconnected) => {
                let child_status = self
                    .child_exit_status_string()
                    .unwrap_or_else(|| "unknown".to_string());
                runtime_log::warn(format!(
                    "[engine_io] line_channel_disconnected; cleanup_runtime child_status={child_status}"
                ));
                self.terminate_locked();
                EngineStdoutPoll::Disconnected { child_status }
            }
        }
    }

    pub(crate) fn drain_until(&self, token: &str, timeout: Duration) -> bool {
        let end = Instant::now() + timeout;
        while Instant::now() < end {
            match self.poll_line(Duration::from_millis(120)) {
                EngineStdoutPoll::Disconnected { .. } => return false,
                EngineStdoutPoll::Tick => {}
                EngineStdoutPoll::Line(line) => {
                    if line.contains(token) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub(crate) fn clear_queue(&self) {
        if let Ok(g) = self.rt.lock()
            && let Some(rt) = g.as_ref()
        {
            while rt.lines.try_recv().is_ok() {}
        }
    }
}
