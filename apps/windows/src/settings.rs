//! The Windows app's own settings, saved in %LOCALAPPDATA%\Volvix Home\config.json next to the server address.

use std::path::PathBuf;

use serde_json::{json, Value};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
use crate::RUN_NAME;

#[derive(Clone)]
pub struct Settings {
    pub server: Option<String>,
    pub start_with_windows: bool,
    pub close_to_tray: bool,
    pub auto_update: bool,
    /// download songs even on a metered connection (off: they wait for an ordinary one)
    pub download_metered: bool,
    /// "original", "high", "normal" or "low" (needs ffmpeg on the server)
    pub stream_quality: String,
    /// the same, or "stream" = as streaming
    pub download_quality: String,
    /// stream "low" while the connection is metered
    pub data_saver: bool,
    /// the first-run default for "start with Windows" has been applied (so turning it off sticks)
    autostart_seen: bool,
}

fn path() -> PathBuf {
    let dir = crate::app_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("config.json")
}

impl Settings {
    pub fn load() -> Settings {
        let v: Value = std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        let flag = |k: &str, d: bool| v.get(k).and_then(|x| x.as_bool()).unwrap_or(d);
        Settings {
            server: v.get("server").and_then(|s| s.as_str()).map(|s| s.trim_end_matches('/').to_string()).filter(|s| !s.is_empty()),
            start_with_windows: flag("startWithWindows", true),
            close_to_tray: flag("closeToTray", true),
            auto_update: flag("autoUpdate", true),
            download_metered: flag("downloadMetered", false),
            stream_quality: v.get("streamQuality").and_then(|x| x.as_str()).unwrap_or("original").to_string(),
            download_quality: v.get("downloadQuality").and_then(|x| x.as_str()).unwrap_or("stream").to_string(),
            data_saver: flag("dataSaver", false),
            autostart_seen: flag("autostartSeen", false),
        }
    }

    fn save(&self) {
        let body = json!({
            "server": self.server, "startWithWindows": self.start_with_windows, "closeToTray": self.close_to_tray,
            "autoUpdate": self.auto_update, "downloadMetered": self.download_metered, "streamQuality": self.stream_quality, "downloadQuality": self.download_quality, "dataSaver": self.data_saver, "autostartSeen": self.autostart_seen
        });
        let _ = std::fs::write(path(), serde_json::to_string_pretty(&body).unwrap_or_default());
    }

    pub fn to_json(&self) -> String {
        json!({ "startWithWindows": self.start_with_windows, "closeToTray": self.close_to_tray, "autoUpdate": self.auto_update }).to_string()
    }

    pub fn set_server(&mut self, url: &str) {
        self.server = Some(url.trim_end_matches('/').to_string());
        self.save();
    }

    pub fn set(&mut self, key: &str, value: bool) {
        match key {
            "startWithWindows" => { self.start_with_windows = value; self.autostart_seen = true; self.apply_autostart(); }
            "closeToTray" => self.close_to_tray = value,
            "autoUpdate" => self.auto_update = value,
            "downloadMetered" => self.download_metered = value,
            "dataSaver" => self.data_saver = value,
            _ => return,
        }
        self.save();
    }

    #[allow(dead_code)] // used by the native app only
    pub fn set_text(&mut self, key: &str, value: &str) {
        match key {
            "streamQuality" => self.stream_quality = value.to_string(),
            "downloadQuality" => self.download_quality = value.to_string(),
            _ => return,
        }
        self.save();
    }

    /// Make Windows' startup entry match the setting. First run: on by default.
    /// (VH_NO_AUTOSTART=1 leaves the registry alone, for tests.)
    pub fn apply_autostart(&mut self) {
        if std::env::var_os("VH_NO_AUTOSTART").is_some() { return; }
        if !self.autostart_seen { self.autostart_seen = true; self.save(); }
        let Ok(exe) = std::env::current_exe() else { return };
        use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE, KEY_READ};
        use winreg::RegKey;
        let Ok((key, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_READ) else { return };
        if self.start_with_windows {
            let _ = key.set_value(RUN_NAME, &format!("\"{}\" --background", exe.display()));
        } else {
            let _ = key.delete_value(RUN_NAME);
        }
    }
}
