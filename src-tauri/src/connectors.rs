use crate::{application::Message, domain::now};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::mpsc,
    time::{sleep, Duration},
};
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LiveEvent {
    Comment {
        id: String,
        user: String,
        text: String,
        timestamp: u64,
        source: String,
    },
    Status {
        status: String,
        message: String,
    },
    Notice {
        message: String,
    },
}
#[async_trait]
pub trait LiveConnector: Send {
    async fn run(
        self: Box<Self>,
        username: String,
        generation: u64,
        tx: mpsc::Sender<Message>,
    ) -> Result<(), String>;
}
pub struct MockLiveConnector;
#[async_trait]
impl LiveConnector for MockLiveConnector {
    async fn run(
        self: Box<Self>,
        _username: String,
        generation: u64,
        tx: mpsc::Sender<Message>,
    ) -> Result<(), String> {
        let _ = tx
            .send(Message::Live(
                generation,
                LiveEvent::Status {
                    status: "connected".into(),
                    message: "Demostración: comentarios simulados, voz real de macOS.".into(),
                },
            ))
            .await;
        let comments = [
            ("CarlosDev", "¿Qué diferencia hay entre Rust y Python?"),
            ("AnaTech", "Muy buena explicación. Gracias por compartir."),
            ("JaviCode", "¿Puedes mostrar un ejemplo de la aplicación?"),
            ("SofiDev", "Saludos desde México 👋"),
            ("Miguel", "Me gusta cómo se ve el modo oscuro."),
            ("Lucía", "¿Cómo funciona la cola de lectura?"),
        ];
        let mut i = 0u64;
        loop {
            sleep(Duration::from_millis(3200)).await;
            let (user, text) = comments[i as usize % comments.len()];
            i += 1;
            let event = LiveEvent::Comment {
                id: format!("demo-{generation}-{i}"),
                user: user.into(),
                text: text.into(),
                timestamp: now(),
                source: "demo".into(),
            };
            if tx.send(Message::Live(generation, event)).await.is_err() {
                return Ok(());
            }
        }
    }
}
pub struct NodeLiveConnector {
    pub node: PathBuf,
    pub script: PathBuf,
}
#[async_trait]
impl LiveConnector for NodeLiveConnector {
    async fn run(
        self: Box<Self>,
        username: String,
        generation: u64,
        tx: mpsc::Sender<Message>,
    ) -> Result<(), String> {
        let mut child = Command::new(&self.node)
            .arg(&self.script)
            .arg(username)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("No se pudo iniciar el puente TikTok: {e}"))?;
        // Keep stdin alive: closing it tells the bridge to disconnect.
        let _input = child.stdin.take();
        let output = child.stdout.take().ok_or("Puente sin salida")?;
        let mut lines = BufReader::new(output).lines();
        while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
            if line.len() > 65_536 {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<LiveEvent>(&line) {
                if tx.send(Message::Live(generation, event)).await.is_err() {
                    return Ok(());
                }
            }
        }
        let status = child.wait().await.map_err(|e| e.to_string())?;
        Err(format!("El puente TikTok terminó ({status})."))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn demo_connects_and_can_be_cancelled() {
        let (tx, mut rx) = mpsc::channel(8);
        let task = tokio::spawn(Box::new(MockLiveConnector).run("ana".into(), 7, tx));
        assert!(
            matches!(rx.recv().await,Some(Message::Live(7,LiveEvent::Status{status,..})) if status=="connected")
        );
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(rx.recv().await.is_none());
    }
}
