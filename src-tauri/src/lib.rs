mod application;
mod connectors;
mod database;
mod domain;
mod speech;
use application::{Action, Actor, Message};
use domain::Snapshot;
use tauri::Manager;
use tokio::sync::{mpsc, oneshot};
struct Backend(mpsc::Sender<Message>);
#[tauri::command]
async fn dispatch(action: Action, backend: tauri::State<'_, Backend>) -> Result<Snapshot, String> {
    let (tx, rx) = oneshot::channel();
    backend
        .0
        .send(Message::Request(action, tx))
        .await
        .map_err(|_| "El servicio local terminó".to_string())?;
    rx.await
        .map_err(|_| "No se recibió respuesta del servicio".to_string())?
}
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            let resources = if cfg!(debug_assertions) {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources")
            } else {
                app.path().resource_dir()?
            };
            let db =
                database::Database::open(&data.join("app.db")).map_err(std::io::Error::other)?;
            let (tx, rx) = mpsc::channel(512);
            let mut actor = Actor::new(
                db,
                resources.clone(),
                tx.clone(),
                Some(app.handle().clone()),
            )
            .map_err(std::io::Error::other)?;
            app.manage(Backend(tx.clone()));
            tauri::async_runtime::spawn(async move {
                match speech::MacSpeechEngine::start(&resources.join("speech-helper"), tx).await {
                    Ok(engine) => actor.set_speech(Box::new(engine)),
                    Err(error) => actor.state.message = error,
                }
                actor.run(rx).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![dispatch])
        .run(tauri::generate_context!())
        .expect("No se pudo iniciar TikTok Live TTS");
}
