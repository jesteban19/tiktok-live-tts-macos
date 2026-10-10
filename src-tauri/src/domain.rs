use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Comment {
    pub id: String,
    pub user: String,
    pub text: String,
    pub timestamp: u64,
    pub source: String,
    #[serde(default = "comment_kind")]
    pub kind: String,
    #[serde(default = "received")]
    pub status: String,
}
fn received() -> String {
    "received".into()
}
fn comment_kind() -> String {
    "comment".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gift {
    pub id: String,
    pub user: String,
    pub name: String,
    pub count: u64,
    pub timestamp: u64,
    pub source: String,
    #[serde(default = "received")]
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub username: String,
    pub voice: String,
    pub rate: f32,
    pub volume: f32,
    pub automatic: bool,
    pub read_username: bool,
    pub read_gifts: bool,
    pub max_queue: usize,
    pub max_age_seconds: u64,
    pub max_length: usize,
    pub blocked_words: Vec<String>,
    pub muted_users: Vec<String>,
    pub ignore_emoji: bool,
    pub deduplicate: bool,
    pub save_history: bool,
    pub auto_scroll: bool,
    pub theme: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            username: String::new(),
            voice: String::new(),
            rate: 0.5,
            volume: 0.85,
            automatic: true,
            read_username: false,
            read_gifts: false,
            max_queue: 20,
            max_age_seconds: 60,
            max_length: 240,
            blocked_words: vec![],
            muted_users: vec![],
            ignore_emoji: true,
            deduplicate: true,
            save_history: false,
            auto_scroll: true,
            theme: "dark".into(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=100).contains(&self.max_queue)
            || !(1..=3600).contains(&self.max_age_seconds)
            || !(1..=2000).contains(&self.max_length)
        {
            return Err("Límites de cola o texto fuera del rango permitido.".into());
        }
        if !self.rate.is_finite()
            || !(0.3..=0.65).contains(&self.rate)
            || !self.volume.is_finite()
            || !(0.0..=1.0).contains(&self.volume)
        {
            return Err("Velocidad o volumen inválido.".into());
        }
        if self.blocked_words.len() > 200
            || self.muted_users.len() > 200
            || self
                .blocked_words
                .iter()
                .chain(self.muted_users.iter())
                .any(|s| s.len() > 100)
        {
            return Err("Las listas admiten hasta 200 entradas de 100 caracteres.".into());
        }
        if !["dark", "light"].contains(&self.theme.as_str()) {
            return Err("Tema inválido.".into());
        }
        Ok(())
    }
}
pub fn username(value: &str) -> Result<String, String> {
    let user = value.trim().trim_start_matches('@');
    if !(2..=32).contains(&user.len())
        || !user
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'.')
    {
        return Err("Introduce un usuario TikTok válido, sin URL.".into());
    }
    Ok(user.to_ascii_lowercase())
}
pub fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
#[derive(Default)]
pub struct Processor {
    recent: VecDeque<(String, u64)>,
}
impl Processor {
    pub fn accept(&mut self, c: &Comment, s: &Settings, time: u64) -> Result<String, String> {
        let text = normalize(&c.text);
        if text.is_empty() {
            return Err("vacío".into());
        }
        if text.chars().count() > s.max_length {
            return Err("demasiado largo".into());
        }
        if s.ignore_emoji && !text.chars().any(char::is_alphanumeric) {
            return Err("solo emojis o símbolos".into());
        }
        if s.muted_users.iter().any(|u| {
            u.trim()
                .trim_start_matches('@')
                .eq_ignore_ascii_case(&c.user)
        }) {
            return Err("usuario silenciado".into());
        }
        let lower = text.to_lowercase();
        if s.blocked_words
            .iter()
            .any(|w| !w.trim().is_empty() && lower.contains(&w.trim().to_lowercase()))
        {
            return Err("palabra bloqueada".into());
        }
        self.recent
            .retain(|(_, at)| time.saturating_sub(*at) < 60_000);
        let key = format!("{}:{}", c.user.to_lowercase(), lower);
        if s.deduplicate && self.recent.iter().any(|(k, _)| k == &key) {
            return Err("repetido".into());
        }
        self.recent.push_back((key, time));
        while self.recent.len() > 500 {
            self.recent.pop_front();
        }
        Ok(text)
    }
}
#[derive(Default)]
pub struct Queue {
    pub pending: VecDeque<Comment>,
}
impl Queue {
    pub fn push(&mut self, comment: Comment, capacity: usize) -> Result<(), String> {
        if self.pending.iter().any(|c| c.id == comment.id) {
            return Err("ya está en cola".into());
        }
        if self.pending.len() >= capacity {
            return Err("cola llena".into());
        }
        self.pending.push_back(comment);
        Ok(())
    }
    pub fn expire(&mut self, time: u64, age_seconds: u64) -> Vec<Comment> {
        let mut expired = vec![];
        self.pending.retain(|c| {
            let keep = time.saturating_sub(c.timestamp) <= age_seconds * 1000;
            if !keep {
                expired.push(c.clone());
            }
            keep
        });
        expired
    }
    pub fn pop(&mut self) -> Option<Comment> {
        self.pending.pop_front()
    }
    pub fn remove(&mut self, id: &str) {
        self.pending.retain(|c| c.id != id);
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub language: String,
}
#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub settings: Settings,
    pub connection: String,
    pub mode: String,
    pub message: String,
    pub comments: Vec<Comment>,
    pub queue: Vec<Comment>,
    pub current: Option<Comment>,
    pub paused: bool,
    pub voices: Vec<Voice>,
    pub speech_ready: bool,
    pub history: Vec<Comment>,
    pub received: u64,
    pub filtered: u64,
    pub played: u64,
    pub viewer_count: Option<u64>,
    pub gifts: Vec<Gift>,
    pub gift_count: u64,
    pub revision: u64,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            connection: "disconnected".into(),
            mode: "demo".into(),
            message: "Listo para conectar".into(),
            comments: vec![],
            queue: vec![],
            current: None,
            paused: false,
            voices: vec![],
            speech_ready: false,
            history: vec![],
            received: 0,
            filtered: 0,
            played: 0,
            viewer_count: None,
            gifts: vec![],
            gift_count: 0,
            revision: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(id: &str, text: &str) -> Comment {
        Comment {
            id: id.into(),
            user: "ana".into(),
            text: text.into(),
            timestamp: 1000,
            source: "demo".into(),
            kind: "comment".into(),
            status: received(),
        }
    }
    #[test]
    fn filters_and_normalization() {
        let mut p = Processor::default();
        let mut s = Settings::default();
        assert_eq!(
            p.accept(&c("1", " Hola  mundo\n"), &s, 1000).unwrap(),
            "Hola mundo"
        );
        assert!(p.accept(&c("2", "hola mundo"), &s, 1001).is_err());
        assert!(p.accept(&c("3", "😂✨"), &s, 1000).is_err());
        s.blocked_words = vec!["spam".into()];
        assert!(p.accept(&c("4", "SPAM"), &s, 1000).is_err());
        s.muted_users = vec!["@ANA".into()];
        assert!(p.accept(&c("5", "hola"), &s, 1000).is_err());
    }
    #[test]
    fn duplicate_window_expires() {
        let mut p = Processor::default();
        let s = Settings::default();
        assert!(p.accept(&c("1", "Hola"), &s, 1000).is_ok());
        assert!(p.accept(&c("2", "Hola"), &s, 61001).is_ok());
    }
    #[test]
    fn fifo_and_capacity() {
        let mut q = Queue::default();
        q.push(c("1", "a"), 2).unwrap();
        q.push(c("2", "b"), 2).unwrap();
        assert!(q.push(c("3", "c"), 2).is_err());
        assert_eq!(q.pop().unwrap().id, "1");
        assert_eq!(q.pop().unwrap().id, "2");
        assert!(q.pop().is_none());
    }
    #[test]
    fn expiration_and_remove() {
        let mut q = Queue::default();
        q.push(c("1", "a"), 2).unwrap();
        assert_eq!(q.expire(62000, 60).len(), 1);
        q.push(c("2", "b"), 2).unwrap();
        q.remove("2");
        assert!(q.pop().is_none());
    }
    #[test]
    fn validate_settings_and_username() {
        let mut s = Settings::default();
        assert!(s.validate().is_ok());
        s.max_queue = 0;
        assert!(s.validate().is_err());
        assert_eq!(username(" @Ana.Dev ").unwrap(), "ana.dev");
        assert!(username("../../tmp").is_err());
    }
    #[test]
    fn empty_and_unicode_length() {
        let mut p = Processor::default();
        let mut s = Settings::default();
        assert!(p.accept(&c("1", " \n"), &s, 1000).is_err());
        s.max_length = 2;
        assert!(p.accept(&c("2", "ñéí"), &s, 1000).is_err());
        assert!(p.accept(&c("3", "ñé"), &s, 1000).is_ok());
    }
    #[test]
    fn older_saved_settings_and_comments_migrate_with_safe_defaults() {
        let settings: Settings =
            serde_json::from_str(r#"{"username":"ana","automatic":true}"#).unwrap();
        assert!(!settings.read_username);
        assert!(!settings.read_gifts);
        let comment: Comment = serde_json::from_str(r#"{"id":"old","user":"ana","text":"hola","timestamp":1,"source":"real","status":"played"}"#).unwrap();
        assert_eq!(comment.kind, "comment");
    }
}
