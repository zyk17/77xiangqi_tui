use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::engine_core::UciUcciEngine;
use super::engine_path::has_non_ascii;
use super::handshake_plan::handshake_protocol_sequence;
use crate::engine::protocol::EngineProtocol;
use crate::engine::protocol::ucci::Ucci;
use crate::engine::protocol::uci::Uci;
use crate::runtime_log;

const KEY_PROTOCOL: &str = "engine_protocol_detected";
const KEY_PROTOCOL_PATH: &str = "engine_protocol_detected_for_path";

fn set_config_kv(lines: &mut Vec<String>, key: &str, value: &str) {
    let prefix = format!("{key}=");
    if let Some(idx) = lines
        .iter()
        .position(|line| line.trim().starts_with(&prefix))
    {
        lines[idx] = format!("{key}={value}");
    } else {
        lines.push(format!("{key}={value}"));
    }
}

fn write_protocol_cue(cfg_path: &Path, proto_id: &str, engine_path: &str) -> std::io::Result<()> {
    let mut lines: Vec<String> = fs::read_to_string(cfg_path)
        .ok()
        .map(|content| {
            content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    set_config_kv(&mut lines, KEY_PROTOCOL, proto_id);
    set_config_kv(&mut lines, KEY_PROTOCOL_PATH, engine_path);
    let body = if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    };
    fs::write(cfg_path, body)
}

impl UciUcciEngine {
    fn persist_protocol_cue(&self, proto_id: &str) {
        if proto_id != "uci" && proto_id != "ucci" {
            return;
        }
        let cfg_path = match self.engine_config_path.lock().ok().and_then(|g| g.clone()) {
            Some(p) => p,
            None => return,
        };
        let Some(engine_path) = self.engine_path.as_ref().filter(|p| !p.is_empty()) else {
            return;
        };
        let _ = write_protocol_cue(&cfg_path, proto_id, engine_path);
    }

    pub(crate) fn try_handshake_protocol(&mut self, proto: &dyn EngineProtocol) -> bool {
        self.send_cmd(proto.init_command()).is_ok()
            && self.drain_until(proto.handshake_done_token(), Duration::from_secs(3))
    }

    pub(crate) fn handshake(&mut self) -> Result<(), String> {
        let pref = self
            .protocol_preference
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let hint = self
            .handshake_protocol_hint
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let uci = Uci;
        let ucci = Ucci;
        runtime_log::debug(format!(
            "[engine_handshake] start pref={} hint={:?}",
            pref, hint
        ));
        let seq = handshake_protocol_sequence(&pref, hint.as_deref());
        let mut got: Option<&'static str> = None;
        for (i, proto_id) in seq.iter().enumerate() {
            if i > 0 {
                self.clear_queue();
            }
            let proto: &dyn EngineProtocol = if *proto_id == "uci" { &uci } else { &ucci };
            if self.try_handshake_protocol(proto) {
                got = Some(proto.protocol_id());
                break;
            }
        }
        let Some(proto_id) = got else {
            self.terminate_locked();
            runtime_log::warn("[engine_handshake] fail reason=no_protocol_matched");
            return Err("引擎握手失败：请确认可执行文件为中国象棋引擎且支持 UCI 或 UCCI".into());
        };
        runtime_log::debug(format!(
            "[engine_handshake] ok protocol={} path={:?}",
            proto_id, self.engine_path
        ));
        *self.last_protocol.lock().unwrap() = proto_id.to_string();
        self.persist_protocol_cue(proto_id);
        self.send_cmd(&format!("setoption name Threads value {}", self.threads))?;
        self.send_cmd(&format!("setoption name Hash value {}", self.hash_mb))?;
        self.send_cmd(&format!(
            "setoption name Repetition Rule value {}",
            self.repetition_rule
        ))?;
        self.send_cmd(&format!(
            "setoption name Draw Rule value {}",
            self.draw_rule
        ))?;
        let _ = self.send_cmd(&format!(
            "setoption name Skill Level value {}",
            self.skill_level
        ));
        if let Some(ref p) = self.engine_path {
            let parent = Path::new(p).parent().unwrap_or(Path::new("."));
            let mut sent = false;
            for name in ["pikafish.nnue", "engine.nnue", "libpikafish.nnue.so"] {
                let nn = parent.join(name);
                if nn.is_file() {
                    let s = nn.to_string_lossy().replace('\\', "/");
                    if has_non_ascii(&s) {
                        runtime_log::debug(format!(
                            "[engine_handshake] skip_evalfile_non_ascii path={s}"
                        ));
                        continue;
                    }
                    let _ = self.send_cmd(&format!("setoption name EvalFile value {s}"));
                    sent = true;
                    break;
                }
            }
            if !sent {
                let mut nnue_any: Vec<PathBuf> = Vec::new();
                if let Ok(rd) = fs::read_dir(parent) {
                    for e in rd.flatten() {
                        let path = e.path();
                        if !path.is_file() {
                            continue;
                        }
                        let file_name_low = path
                            .file_name()
                            .and_then(|x| x.to_str())
                            .map(|x| x.to_ascii_lowercase())
                            .unwrap_or_default();
                        let is_nnue = path
                            .extension()
                            .and_then(|x| x.to_str())
                            .map(|x| x.eq_ignore_ascii_case("nnue"))
                            .unwrap_or(false)
                            || file_name_low.contains(".nnue.so");
                        if is_nnue {
                            nnue_any.push(path);
                        }
                    }
                }
                nnue_any.sort();
                if let Some(nn) = nnue_any.first() {
                    let s = nn.to_string_lossy().replace('\\', "/");
                    if has_non_ascii(&s) {
                        runtime_log::debug(format!(
                            "[engine_handshake] skip_evalfile_non_ascii path={s}"
                        ));
                    } else {
                        let _ = self.send_cmd(&format!("setoption name EvalFile value {s}"));
                    }
                }
            }
        }
        self.send_cmd("isready")?;
        if !self.drain_until("readyok", Duration::from_secs(4)) {
            self.terminate_locked();
            return Err("isready timeout".into());
        }
        Ok(())
    }
}
