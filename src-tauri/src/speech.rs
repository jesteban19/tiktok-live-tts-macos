use crate::application::Message;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::mpsc,
};
#[async_trait]
pub trait SpeechEngine: Send {
    async fn send(&mut self, message: Value) -> Result<(), String>;
}
pub struct MacSpeechEngine {
    input: ChildStdin,
    _child: Child,
}
impl MacSpeechEngine {
    pub async fn start(path: &Path, tx: mpsc::Sender<Message>) -> Result<Self, String> {
        let mut child = Command::new(path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("No se pudo iniciar la voz nativa: {e}"))?;
        let input = child.stdin.take().ok_or("Motor sin entrada")?;
        let output = child.stdout.take().ok_or("Motor sin salida")?;
        tokio::spawn(async move {
            let mut lines = BufReader::new(output).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(value) = serde_json::from_str(&line) {
                    if tx.send(Message::Speech(value)).await.is_err() {
                        return;
                    }
                }
            }
            let _=tx.send(Message::Speech(json!({"kind":"unavailable","message":"El motor de voz terminó. Reinicia la aplicación."}))).await;
        });
        Ok(Self {
            input,
            _child: child,
        })
    }
}
#[async_trait]
impl SpeechEngine for MacSpeechEngine {
    async fn send(&mut self, message: Value) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(&message).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        self.input
            .write_all(&bytes)
            .await
            .map_err(|e| e.to_string())
    }
}
