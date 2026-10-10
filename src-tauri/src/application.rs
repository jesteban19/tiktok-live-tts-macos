use crate::{
    connectors::{LiveConnector, LiveEvent, MockLiveConnector, NodeLiveConnector},
    database::Database,
    domain::*,
    speech::SpeechEngine,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::VecDeque, path::PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Snapshot,
    Connect { username: String, mode: String },
    Disconnect,
    Settings { settings: Settings },
    Pause,
    Resume,
    Stop,
    Next,
    Enqueue { id: String },
    Remove { id: String },
    ClearView,
    ClearQueue,
    TestVoice,
    History,
    ClearHistory,
}
pub enum Message {
    Request(Action, oneshot::Sender<Result<Snapshot, String>>),
    Live(u64, LiveEvent),
    Speech(Value),
    ConnectorError(u64, String),
}
pub struct Actor {
    pub state: Snapshot,
    pub queue: Queue,
    processor: Processor,
    db: Database,
    speech: Option<Box<dyn SpeechEngine>>,
    generation: u64,
    live_task: Option<JoinHandle<()>>,
    resources: PathBuf,
    tx: mpsc::Sender<Message>,
    app: Option<AppHandle>,
    speech_token: Option<String>,
    utterance: u64,
    session_started: u64,
    seen: VecDeque<String>,
}
impl Actor {
    pub fn new(
        db: Database,
        resources: PathBuf,
        tx: mpsc::Sender<Message>,
        app: Option<AppHandle>,
    ) -> Result<Self, String> {
        let state = Snapshot {
            settings: db.load()?,
            ..Snapshot::default()
        };
        Ok(Self {
            state,
            queue: Queue::default(),
            processor: Processor::default(),
            db,
            speech: None,
            generation: 0,
            live_task: None,
            resources,
            tx,
            app,
            speech_token: None,
            utterance: 0,
            session_started: now(),
            seen: VecDeque::new(),
        })
    }
    pub fn set_speech(&mut self, speech: Box<dyn SpeechEngine>) {
        self.speech = Some(speech);
    }
    pub fn publish(&mut self) -> Snapshot {
        self.state.queue = self.queue.pending.iter().cloned().collect();
        self.state.revision += 1;
        if let Some(app) = &self.app {
            let _ = app.emit("app:state", &self.state);
        }
        self.state.clone()
    }
    fn status(&mut self, id: &str, status: &str) {
        if let Some(gift) = self.state.gifts.iter_mut().find(|gift| gift.id == id) {
            gift.status = status.into();
        }
        if let Some(c) = self.state.comments.iter_mut().find(|c| c.id == id) {
            c.status = status.into();
        }
        if self.state.settings.save_history {
            let candidate = self
                .state
                .comments
                .iter()
                .find(|c| c.id == id)
                .cloned()
                .or_else(|| self.state.current.as_ref().filter(|c| c.id == id).cloned());
            if let Some(mut c) = candidate.filter(|c| c.kind == "comment") {
                c.status = status.into();
                if let Err(e) = self.db.record(&c) {
                    self.state.message = format!("Error guardando historial: {e}");
                }
            }
        }
    }
    fn finish_current(&mut self, status: &str) {
        if let Some(c) = self.state.current.clone() {
            self.status(&c.id, status);
            self.state.current = None;
        }
    }
    async fn voice(&mut self, value: Value) -> Result<(), String> {
        self.speech
            .as_mut()
            .ok_or("Motor de voz no disponible")?
            .send(value)
            .await
    }
    fn invalidate_connection(&mut self) {
        self.generation += 1;
        if let Some(task) = self.live_task.take() {
            task.abort();
        }
    }
    pub async fn action(&mut self, action: Action) -> Result<Snapshot, String> {
        match action {
            Action::Snapshot => {}
            Action::Connect {
                username: input,
                mode,
            } => {
                let user = if mode == "demo" && input.trim().is_empty() {
                    "demo".into()
                } else {
                    username(&input)?
                };
                if !["demo", "real"].contains(&mode.as_str()) {
                    return Err("Modo de conexión inválido".into());
                }
                let mut s = self.state.settings.clone();
                s.username = user.clone();
                self.db.save(&s)?;
                if self.speech.is_some() {
                    self.voice(json!({"command":"stop"})).await?;
                }
                self.speech_token = None;
                self.finish_current("detenido");
                self.clear_queue();
                self.invalidate_connection();
                self.session_started = now();
                self.seen.clear();
                self.processor = Processor::default();
                self.state.comments.clear();
                self.state.gifts.clear();
                self.state.gift_count = 0;
                self.state.viewer_count = None;
                self.state.received = 0;
                self.state.filtered = 0;
                self.state.played = 0;
                self.state.settings = s;
                self.state.mode = mode.clone();
                self.state.connection = "connecting".into();
                self.state.message = if mode == "demo" {
                    "Iniciando demostración".into()
                } else {
                    "Conectando a TikTok LIVE; puede requerir firma Euler Stream.".into()
                };
                let connector: Box<dyn LiveConnector> = if mode == "demo" {
                    Box::new(MockLiveConnector)
                } else {
                    Box::new(NodeLiveConnector {
                        node: self.resources.join("node"),
                        script: self.resources.join("bridge.cjs"),
                    })
                };
                let tx = self.tx.clone();
                let generation = self.generation;
                self.live_task = Some(tokio::spawn(async move {
                    if let Err(error) = connector.run(user, generation, tx.clone()).await {
                        let _ = tx.send(Message::ConnectorError(generation, error)).await;
                    }
                }));
            }
            Action::Disconnect => {
                self.invalidate_connection();
                self.state.connection = "disconnected".into();
                self.state.viewer_count = None;
                self.state.message =
                    "Desconectado. La cola pendiente puede seguir leyéndose.".into();
            }
            Action::Settings { settings } => {
                settings.validate()?;
                if !settings.voice.is_empty()
                    && !self.state.voices.iter().any(|v| v.id == settings.voice)
                {
                    return Err("La voz no está instalada.".into());
                }
                self.db.save(&settings)?;
                self.state.settings = settings;
                while self.queue.pending.len() > self.state.settings.max_queue {
                    if let Some(c) = self.queue.pending.pop_back() {
                        self.status(&c.id, "descartado: límite de cola");
                    }
                }
            }
            Action::Pause => {
                self.voice(json!({"command":"pause"})).await?;
                self.state.paused = true;
            }
            Action::Resume => {
                self.voice(json!({"command":"resume"})).await?;
                self.state.paused = false;
            }
            Action::Stop => {
                self.voice(json!({"command":"stop"})).await?;
                self.speech_token = None;
                self.finish_current("detenido");
                self.state.paused = true;
                self.clear_queue();
            }
            Action::Next => {
                self.voice(json!({"command":"stop"})).await?;
                self.speech_token = None;
                self.finish_current("saltado");
                self.state.paused = false;
            }
            Action::Enqueue { id } => {
                if self.state.current.as_ref().is_some_and(|c| c.id == id) {
                    return Err("Este comentario ya se está leyendo.".into());
                }
                let c = self
                    .state
                    .comments
                    .iter()
                    .find(|c| c.id == id)
                    .cloned()
                    .ok_or("Comentario fuera de la vista reciente")?;
                if c.status.starts_with("filtrado:") {
                    return Err("Este comentario fue bloqueado por los filtros.".into());
                }
                let mut c = c;
                c.timestamp = now();
                self.queue.push(c, self.state.settings.max_queue)?;
                self.status(&id, "queued");
            }
            Action::Remove { id } => {
                self.queue.remove(&id);
                self.status(&id, "retirado");
            }
            Action::ClearView => {
                self.state.comments.clear();
            }
            Action::ClearQueue => self.clear_queue(),
            Action::TestVoice => {
                if !self.state.speech_ready {
                    return Err("El motor de voz aún no está listo.".into());
                }
                let c = Comment {
                    id: format!("test-{}", now()),
                    user: "Prueba de voz".into(),
                    text: "Hola. Esta es la voz de TikTok Live TTS.".into(),
                    timestamp: now(),
                    source: "test".into(),
                    kind: "comment".into(),
                    status: "queued".into(),
                };
                self.queue.push(c, self.state.settings.max_queue)?;
                self.state.paused = false;
            }
            Action::History => {
                self.state.history = self.db.history()?;
            }
            Action::ClearHistory => {
                self.db.clear()?;
                self.state.history.clear();
            }
        }
        self.tick().await;
        Ok(self.publish())
    }
    fn clear_queue(&mut self) {
        let ids: Vec<_> = self.queue.pending.drain(..).map(|c| c.id).collect();
        for id in ids {
            self.status(&id, "retirado");
        }
    }
    pub fn receive(&mut self, mut c: Comment) {
        if c.id.len() > 200 || c.user.len() > 200 || c.text.len() > 16_000 {
            return;
        }
        // Transport IDs are idempotent even when textual deduplication is disabled.
        if !self.fresh(&c.id, c.timestamp, &c.source) {
            return;
        }
        self.state.received += 1;
        match self.processor.accept(&c, &self.state.settings, now()) {
            Ok(text) => {
                c.text = text;
                if self.state.settings.automatic {
                    c.status = "queued".into();
                    if let Err(reason) = self.queue.push(c.clone(), self.state.settings.max_queue) {
                        c.status = format!("descartado: {reason}");
                    }
                }
            }
            Err(reason) => {
                self.state.filtered += 1;
                c.status = format!("filtrado: {reason}");
            }
        }
        if self.state.settings.save_history {
            if let Err(e) = self.db.record(&c) {
                self.state.message = format!("Error guardando historial: {e}");
            }
        }
        self.state.comments.push(c);
        if self.state.comments.len() > 300 {
            self.state.comments.remove(0);
        }
    }
    fn fresh(&mut self, id: &str, timestamp: u64, source: &str) -> bool {
        if (source == "real" && timestamp < self.session_started)
            || self.seen.iter().any(|old| old == id)
        {
            return false;
        }
        self.seen.push_back(id.into());
        if self.seen.len() > 5000 {
            self.seen.pop_front();
        }
        true
    }
    pub fn receive_gift(&mut self, mut gift: Gift) {
        if gift.id.len() > 200
            || gift.user.len() > 200
            || gift.name.len() > 500
            || gift.count == 0
            || gift.count > 1_000_000
            || !self.fresh(&gift.id, gift.timestamp, &gift.source)
        {
            return;
        }
        self.state.gift_count = self.state.gift_count.saturating_add(gift.count);
        if self.state.settings.read_gifts {
            let mut c = Comment {
                id: gift.id.clone(),
                user: gift.user.clone(),
                text: format!("Regalo recibido: {} {}.", gift.count, gift.name),
                timestamp: gift.timestamp,
                source: gift.source.clone(),
                kind: "gift".into(),
                status: "queued".into(),
            };
            // Apply moderation to announcements too; gifts remain visible and counted.
            let gift_settings = Settings {
                deduplicate: false,
                ..self.state.settings.clone()
            };
            match self.processor.accept(&c, &gift_settings, now()) {
                Ok(text) => {
                    c.text = text;
                    gift.status = match self.queue.push(c, self.state.settings.max_queue) {
                        Ok(()) => "queued".into(),
                        Err(reason) => format!("descartado: {reason}"),
                    };
                }
                Err(reason) => gift.status = format!("filtrado: {reason}"),
            }
        }
        self.state.gifts.push(gift);
        if self.state.gifts.len() > 100 {
            self.state.gifts.remove(0);
        }
    }
    fn receive_live(&mut self, event: LiveEvent) {
        match event {
            LiveEvent::Comment {
                id,
                user,
                text,
                timestamp,
                source,
            } => self.receive(Comment {
                id,
                user,
                text,
                timestamp,
                source,
                kind: "comment".into(),
                status: "received".into(),
            }),
            LiveEvent::Gift { gift } => self.receive_gift(gift),
            LiveEvent::Viewers { count } => {
                if self.state.connection == "connected" {
                    self.state.viewer_count = Some(count);
                }
            }
            LiveEvent::Status { status, message } => {
                if status == "connected" {
                    self.session_started = now();
                }
                self.state.viewer_count = None;
                self.state.connection = status;
                self.state.message = message;
            }
            LiveEvent::Notice { message } => self.state.message = message,
        }
    }
    fn speech_text(&self, c: &Comment) -> String {
        if self.state.settings.read_username && c.source != "test" {
            if c.kind == "gift" {
                format!("{}. {}", c.user, c.text)
            } else {
                format!("{} dice: {}", c.user, c.text)
            }
        } else {
            c.text.clone()
        }
    }
    pub async fn tick(&mut self) {
        for c in self
            .queue
            .expire(now(), self.state.settings.max_age_seconds)
        {
            self.status(&c.id, "descartado: antiguo");
        }
        if self.state.paused || self.state.current.is_some() || !self.state.speech_ready {
            return;
        }
        if let Some(mut c) = self.queue.pop() {
            self.utterance += 1;
            let token = format!("speech-{}", self.utterance);
            let payload = json!({"command":"speak","id":token,"text":self.speech_text(&c),"voice":self.state.settings.voice,"rate":self.state.settings.rate,"volume":self.state.settings.volume});
            match self.voice(payload).await {
                Ok(()) => {
                    c.status = "speaking".into();
                    self.status(&c.id, "speaking");
                    self.speech_token = Some(token);
                    self.state.current = Some(c);
                }
                Err(error) => {
                    self.state.message = error;
                    self.state.speech_ready = false;
                    self.status(&c.id, "error de voz");
                }
            }
        }
    }
    pub async fn handle_speech(&mut self, value: Value) {
        match value["kind"].as_str().unwrap_or("") {
            "voices" => {
                if let Ok(voices) = serde_json::from_value(value["voices"].clone()) {
                    self.state.voices = voices;
                    self.state.speech_ready = true;
                }
            }
            "finished" | "cancelled" | "error" => {
                if value["id"].as_str() == self.speech_token.as_deref() {
                    let kind = value["kind"].as_str().unwrap_or("");
                    if self.state.current.is_some() {
                        if kind == "finished" {
                            self.state.played += 1;
                            self.finish_current("played");
                        } else {
                            self.finish_current("error de voz");
                        }
                    }
                    self.speech_token = None;
                    if kind == "error" {
                        self.state.message =
                            value["message"].as_str().unwrap_or("Error de voz").into();
                    } else if kind == "cancelled" {
                        self.state.message = "macOS canceló la lectura. Comprueba la voz instalada y los permisos de ejecución.".into();
                        self.state.paused = true;
                    }
                }
            }
            "unavailable" => {
                self.state.speech_ready = false;
                self.speech_token = None;
                self.finish_current("error de voz");
                self.state.message = value["message"]
                    .as_str()
                    .unwrap_or("Motor no disponible")
                    .into();
            }
            _ => {}
        }
    }
    pub async fn run(mut self, mut rx: mpsc::Receiver<Message>) {
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            tokio::select! {
                message=rx.recv()=>{let Some(message)=message else{break};match message {
                    Message::Request(action,reply)=>{let result=self.action(action).await;let _=reply.send(result);continue;},
                    Message::Live(g,event) if g==self.generation=>self.receive_live(event),
                    Message::ConnectorError(g,error) if g==self.generation=>{self.state.viewer_count=None;if !["ended","offline"].contains(&self.state.connection.as_str()){self.state.connection="error".into();self.state.message=error;}},
                    Message::Speech(value)=>self.handle_speech(value).await,
                    _=>continue,
                }self.tick().await;self.publish();},
                _=timer.tick()=>{let previous=self.queue.pending.len();self.tick().await;if previous!=self.queue.pending.len(){self.publish();}}
            }
        }
    }
}
impl Drop for Actor {
    fn drop(&mut self) {
        if let Some(task) = &self.live_task {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    struct FakeSpeech(Arc<Mutex<Vec<Value>>>);
    #[async_trait]
    impl SpeechEngine for FakeSpeech {
        async fn send(&mut self, v: Value) -> Result<(), String> {
            self.0.lock().unwrap().push(v);
            Ok(())
        }
    }
    fn fixture() -> (Actor, Arc<Mutex<Vec<Value>>>) {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let (tx, _) = mpsc::channel(16);
        let mut actor = Actor::new(db, PathBuf::new(), tx, None).unwrap();
        let calls = Arc::new(Mutex::new(vec![]));
        actor.set_speech(Box::new(FakeSpeech(calls.clone())));
        actor.state.speech_ready = true;
        (actor, calls)
    }
    fn c(id: &str) -> Comment {
        Comment {
            id: id.into(),
            user: "ana".into(),
            text: format!("Comentario {id}"),
            timestamp: now(),
            source: "demo".into(),
            kind: "comment".into(),
            status: "received".into(),
        }
    }
    #[tokio::test]
    async fn completion_controls_fifo_without_overlap() {
        let (mut a, calls) = fixture();
        a.receive(c("1"));
        a.receive(c("2"));
        a.tick().await;
        a.tick().await;
        assert_eq!(calls.lock().unwrap().len(), 1);
        assert_eq!(a.state.current.as_ref().unwrap().id, "1");
        a.handle_speech(json!({"kind":"finished","id":"speech-1"}))
            .await;
        a.tick().await;
        assert_eq!(a.state.current.as_ref().unwrap().id, "2");
        assert_eq!(a.state.played, 1);
    }
    #[tokio::test]
    async fn pause_resume_stop_and_stale_completion() {
        let (mut a, _) = fixture();
        a.receive(c("1"));
        a.receive(c("2"));
        a.tick().await;
        a.action(Action::Pause).await.unwrap();
        assert!(a.state.paused);
        a.action(Action::Resume).await.unwrap();
        assert!(!a.state.paused);
        a.action(Action::Next).await.unwrap();
        assert_eq!(a.state.current.as_ref().unwrap().id, "2");
        a.handle_speech(json!({"kind":"cancelled","id":"speech-1"}))
            .await;
        assert_eq!(a.state.current.as_ref().unwrap().id, "2");
        a.action(Action::Stop).await.unwrap();
        assert!(a.state.current.is_none());
        assert!(a.queue.pending.is_empty());
        assert!(a.state.paused);
    }
    #[tokio::test]
    async fn manual_and_filtered_comments() {
        let (mut a, calls) = fixture();
        a.state.settings.automatic = false;
        a.state.settings.blocked_words = vec!["spam".into()];
        a.receive(c("1"));
        a.receive(Comment {
            text: "spam".into(),
            ..c("2")
        });
        a.tick().await;
        assert!(calls.lock().unwrap().is_empty());
        assert!(a.action(Action::Enqueue { id: "2".into() }).await.is_err());
        a.action(Action::Enqueue { id: "1".into() }).await.unwrap();
        assert_eq!(a.state.current.as_ref().unwrap().id, "1");
    }
    #[tokio::test]
    async fn invalid_mode_does_not_connect() {
        let (mut a, _) = fixture();
        assert!(a
            .action(Action::Connect {
                username: "ana".into(),
                mode: "fake-real".into()
            })
            .await
            .is_err());
        assert_eq!(a.state.connection, "disconnected");
        a.action(Action::Disconnect).await.unwrap();
        assert_eq!(a.state.connection, "disconnected");
    }
    #[tokio::test]
    async fn history_completion_survives_cleared_view() {
        let (mut a, _) = fixture();
        a.state.settings.save_history = true;
        a.receive(c("1"));
        a.tick().await;
        a.state.comments.clear();
        a.handle_speech(json!({"kind":"finished","id":"speech-1"}))
            .await;
        assert_eq!(a.db.history().unwrap()[0].status, "played");
    }
    #[tokio::test]
    async fn unexpected_native_cancellation_pauses_queue() {
        let (mut a, _) = fixture();
        a.receive(c("1"));
        a.receive(c("2"));
        a.tick().await;
        a.handle_speech(json!({"kind":"cancelled","id":"speech-1"}))
            .await;
        a.tick().await;
        assert!(a.state.paused);
        assert!(a.state.current.is_none());
        assert_eq!(a.queue.pending.len(), 1);
    }
    #[tokio::test]
    async fn username_option_changes_audio_only_and_skips_voice_test() {
        let (mut a, calls) = fixture();
        a.receive(c("1"));
        a.tick().await;
        assert_eq!(calls.lock().unwrap()[0]["text"], "Comentario 1");
        a.handle_speech(json!({"kind":"finished","id":"speech-1"}))
            .await;
        a.state.settings.read_username = true;
        a.receive(c("2"));
        a.tick().await;
        assert_eq!(calls.lock().unwrap()[1]["text"], "ana dice: Comentario 2");
        assert_eq!(a.state.comments[1].text, "Comentario 2");
        assert_eq!(
            a.speech_text(&Comment {
                source: "test".into(),
                ..c("3")
            }),
            "Comentario 3"
        );
    }
    fn gift(id: &str) -> Gift {
        Gift {
            id: id.into(),
            user: "ana".into(),
            name: "Rosa".into(),
            count: 3,
            timestamp: now(),
            source: "demo".into(),
            status: "received".into(),
        }
    }
    #[tokio::test]
    async fn gifts_count_once_and_share_fifo_without_counting_as_comments() {
        let (mut a, calls) = fixture();
        a.receive_gift(gift("gift:1"));
        assert_eq!(a.state.gift_count, 3);
        assert!(a.queue.pending.is_empty());
        a.state.settings.read_gifts = true;
        a.state.settings.read_username = true;
        a.receive(c("1"));
        a.receive_gift(gift("gift:2"));
        a.receive_gift(gift("gift:2"));
        a.receive_gift(gift("gift:3"));
        assert_eq!(a.state.gift_count, 9);
        assert_eq!(a.state.received, 1);
        assert_eq!(a.queue.pending.len(), 3);
        a.tick().await;
        assert_eq!(a.state.current.as_ref().unwrap().id, "1");
        a.handle_speech(json!({"kind":"finished","id":"speech-1"}))
            .await;
        a.tick().await;
        assert_eq!(
            calls.lock().unwrap()[1]["text"],
            "ana. Regalo recibido: 3 Rosa."
        );
        a.handle_speech(json!({"kind":"finished","id":"speech-2"}))
            .await;
        assert_eq!(a.state.gifts[1].status, "played");
        assert_eq!(a.state.gifts[2].status, "queued");
    }
    #[test]
    fn gift_announcements_respect_muted_users() {
        let (mut a, _) = fixture();
        a.state.settings.read_gifts = true;
        a.state.settings.muted_users = vec!["ana".into()];
        a.receive_gift(gift("gift:1"));
        assert_eq!(a.state.gift_count, 3);
        assert!(a.queue.pending.is_empty());
        assert_eq!(a.state.gifts[0].status, "filtrado: usuario silenciado");
    }
    #[test]
    fn old_live_messages_are_rejected_without_rewriting_original_time() {
        let (mut a, _) = fixture();
        a.session_started = 2000;
        a.receive(Comment {
            source: "real".into(),
            timestamp: 1000,
            ..c("old")
        });
        a.receive(Comment {
            source: "real".into(),
            timestamp: 0,
            ..c("unknown")
        });
        a.receive_gift(Gift {
            source: "real".into(),
            timestamp: 1000,
            ..gift("gift:old")
        });
        assert!(a.state.comments.is_empty());
        assert!(a.state.gifts.is_empty());
        a.receive(Comment {
            source: "real".into(),
            timestamp: 2001,
            ..c("new")
        });
        assert_eq!(a.state.comments[0].timestamp, 2001);
        a.state.comments.clear();
        a.receive(Comment {
            source: "real".into(),
            timestamp: 2001,
            ..c("new")
        });
        assert_eq!(a.state.received, 1); // clearing the view cannot permit a replay.
    }
    #[tokio::test]
    async fn new_session_clears_previous_audio_queue_and_metrics() {
        let (mut a, calls) = fixture();
        a.receive(c("old"));
        a.receive(c("pending"));
        a.tick().await;
        a.state.viewer_count = Some(42);
        a.receive_gift(gift("gift:old"));
        a.action(Action::Connect {
            username: "demo".into(),
            mode: "demo".into(),
        })
        .await
        .unwrap();
        assert!(a.state.current.is_none());
        assert!(a.queue.pending.is_empty());
        assert!(a.state.comments.is_empty());
        assert!(a.state.gifts.is_empty());
        assert_eq!(a.state.gift_count, 0);
        assert_eq!(a.state.viewer_count, None);
        assert_eq!(calls.lock().unwrap().last().unwrap()["command"], "stop");
    }
    #[test]
    fn viewer_count_is_live_and_unavailable_after_disconnect() {
        let (mut a, _) = fixture();
        a.receive_live(LiveEvent::Viewers { count: 100 });
        assert_eq!(a.state.viewer_count, None);
        a.receive_live(LiveEvent::Status {
            status: "connected".into(),
            message: "ok".into(),
        });
        a.receive_live(LiveEvent::Viewers { count: 42 });
        assert_eq!(a.state.viewer_count, Some(42));
        a.receive_live(LiveEvent::Viewers { count: 0 });
        assert_eq!(a.state.viewer_count, Some(0));
        a.receive_live(LiveEvent::Status {
            status: "reconnecting".into(),
            message: "retry".into(),
        });
        assert_eq!(a.state.viewer_count, None);
    }
    #[tokio::test]
    async fn reopening_loads_preferences_and_history_without_speaking_old_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");
        let db = Database::open(&path).unwrap();
        db.save(&Settings {
            save_history: true,
            read_username: true,
            read_gifts: true,
            ..Settings::default()
        })
        .unwrap();
        db.record(&c("saved-old")).unwrap();
        drop(db);
        let (tx, _) = mpsc::channel(16);
        let mut a = Actor::new(Database::open(&path).unwrap(), PathBuf::new(), tx, None).unwrap();
        let calls = Arc::new(Mutex::new(vec![]));
        a.set_speech(Box::new(FakeSpeech(calls.clone())));
        a.state.speech_ready = true;
        a.action(Action::History).await.unwrap();
        assert!(a.state.settings.read_username);
        assert!(a.state.settings.read_gifts);
        assert_eq!(a.state.history.len(), 1);
        assert!(a.state.comments.is_empty());
        assert!(a.queue.pending.is_empty());
        assert!(calls.lock().unwrap().is_empty());
        a.receive(Comment {
            timestamp: a.session_started.saturating_sub(1),
            source: "real".into(),
            ..c("replayed")
        });
        a.tick().await;
        assert!(calls.lock().unwrap().is_empty());
    }
}
