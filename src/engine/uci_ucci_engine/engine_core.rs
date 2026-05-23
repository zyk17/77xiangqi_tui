use std::path::PathBuf;
use std::sync::Mutex;

use super::engine_path::{same_engine_path, sanitize_engine_path};
use super::process::EngineRt;
use super::types::EngineConfigureRequest;

pub struct UciUcciEngine {
    pub(crate) engine_path: Option<String>,
    pub threads: i32,
    pub hash_mb: i32,
    pub repetition_rule: String,
    pub draw_rule: String,
    pub skill_level: i32,
    /// `auto`（同 `prefer_ucci`：先试 UCCI 再 UCI）| `prefer_ucci` | `prefer_uci` | `uci_only` | `ucci_only`
    pub(crate) protocol_preference: Mutex<String>,
    pub(crate) last_protocol: Mutex<String>,
    pub(crate) rt: Mutex<Option<EngineRt>>,
    pub(crate) engine_config_path: Mutex<Option<PathBuf>>,
    pub(crate) handshake_protocol_hint: Mutex<Option<String>>,
}

impl UciUcciEngine {
    pub fn new(default_path: Option<String>) -> Self {
        Self {
            engine_path: default_path,
            threads: 8,
            hash_mb: 512,
            repetition_rule: "AsianRule".into(),
            draw_rule: "None".into(),
            skill_level: 20,
            protocol_preference: Mutex::new("auto".into()),
            last_protocol: Mutex::new(String::new()),
            rt: Mutex::new(None),
            engine_config_path: Mutex::new(None),
            handshake_protocol_hint: Mutex::new(None),
        }
    }

    pub fn configure(&mut self, req: EngineConfigureRequest) {
        if let Some(p) = req.engine_path {
            self.engine_path = sanitize_engine_path(&p);
        }
        *self.engine_config_path.lock().unwrap() = req.engine_config_path;
        let hint = match (req.protocol_detected_for_path, req.protocol_detected) {
            (Some(ref path), Some(ref proto))
                if self
                    .engine_path
                    .as_ref()
                    .map(|ep| same_engine_path(ep, path))
                    .unwrap_or(false)
                    && (proto == "uci" || proto == "ucci") =>
            {
                Some(proto.clone())
            }
            _ => None,
        };
        *self.handshake_protocol_hint.lock().unwrap() = hint;
        if let Some(t) = req.threads {
            self.threads = t.max(1);
        }
        if let Some(h) = req.hash_mb {
            self.hash_mb = h.max(16);
        }
        if let Some(r) = req.repetition_rule {
            if !r.is_empty() {
                self.repetition_rule = r;
            }
        }
        if let Some(d) = req.draw_rule {
            if !d.is_empty() {
                self.draw_rule = d;
            }
        }
        if let Some(s) = req.skill_level {
            self.skill_level = s.clamp(0, 20);
        }
        if let Some(p) = req.engine_protocol_preference {
            let p = p.trim().to_ascii_lowercase();
            let norm = match p.as_str() {
                "auto" => "auto",
                "uci_only" => "uci_only",
                "ucci_only" => "ucci_only",
                "prefer_ucci" => "prefer_ucci",
                "prefer_uci" => "prefer_uci",
                _ => "",
            };
            if !norm.is_empty() {
                *self.protocol_preference.lock().unwrap() = norm.to_string();
            }
        }
        self.terminate_locked();
    }

    pub fn start(&mut self) {
        if self.rt.lock().map(|g| g.is_some()).unwrap_or(false) {
            return;
        }
        if self.engine_path.is_none() {
            return;
        }
        let _ = self.spawn_process();
    }
}
