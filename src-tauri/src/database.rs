use crate::domain::{Comment, Settings};
use rusqlite::{params, Connection};
use std::path::Path;
pub struct Database {
    connection: Connection,
}
impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|e| e.to_string())?;
        connection.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK (id=1), json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS history (id TEXT PRIMARY KEY, timestamp INTEGER NOT NULL, json TEXT NOT NULL);").map_err(|e|e.to_string())?;
        Ok(Self { connection })
    }
    pub fn load(&self) -> Result<Settings, String> {
        let value = self
            .connection
            .query_row("SELECT json FROM settings WHERE id=1", [], |r| {
                r.get::<_, String>(0)
            });
        match value {
            Ok(json) => {
                let s: Settings = serde_json::from_str(&json).map_err(|e| e.to_string())?;
                s.validate()?;
                Ok(s)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Settings::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn save(&self, s: &Settings) -> Result<(), String> {
        s.validate()?;
        let json = serde_json::to_string(s).map_err(|e| e.to_string())?;
        self.connection.execute("INSERT INTO settings(id,json) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",params![json]).map_err(|e|e.to_string())?;
        Ok(())
    }
    pub fn record(&self, c: &Comment) -> Result<(), String> {
        let json = serde_json::to_string(c).map_err(|e| e.to_string())?;
        self.connection.execute("INSERT INTO history(id,timestamp,json) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET json=excluded.json",params![c.id,c.timestamp,json]).map_err(|e|e.to_string())?;
        self.connection.execute("DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY timestamp DESC LIMIT 5000)",[]).map_err(|e|e.to_string())?;
        Ok(())
    }
    pub fn history(&self) -> Result<Vec<Comment>, String> {
        let mut stmt = self
            .connection
            .prepare("SELECT json FROM history ORDER BY timestamp DESC LIMIT 1000")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            let json = r.map_err(|e| e.to_string())?;
            serde_json::from_str(&json).map_err(|e| e.to_string())
        })
        .collect()
    }
    pub fn clear(&self) -> Result<(), String> {
        self.connection
            .execute("DELETE FROM history", [])
            .map_err(|e| e.to_string())?;
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn survives_reopen_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");
        {
            let db = Database::open(&path).unwrap();
            let s = Settings {
                username: "ana".into(),
                save_history: true,
                read_username: true,
                read_gifts: true,
                ..Settings::default()
            };
            db.save(&s).unwrap();
            db.record(&Comment {
                id: "1".into(),
                user: "ana".into(),
                text: "hola".into(),
                timestamp: 1,
                source: "demo".into(),
                kind: "comment".into(),
                status: "played".into(),
            })
            .unwrap();
        }
        let db = Database::open(&path).unwrap();
        assert_eq!(db.load().unwrap().username, "ana");
        assert!(db.load().unwrap().read_username);
        assert!(db.load().unwrap().read_gifts);
        assert_eq!(db.history().unwrap().len(), 1);
        db.clear().unwrap();
        assert!(db.history().unwrap().is_empty());
    }
}
