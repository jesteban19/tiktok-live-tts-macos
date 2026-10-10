# Validación — 10 de octubre de 2026

Validaciones en macOS Apple Silicon. Las rutas de desarrollo y las herramientas locales del autor no son requisitos para compilar desde una copia nueva.

## Versión 0.2.0: nombres, espectadores, regalos y comentarios nuevos

| Comprobación | Resultado |
| --- | --- |
| Frontend | TypeScript y Vite OK; 1 prueba Vitest OK |
| Puente TikTok | 9 pruebas Node OK: contratos proto v3, hora original, historial, reconexión, IDs, espectadores actuales y rachas de regalos |
| Backend | 22 pruebas Rust OK: TTS con/sin nombre, cola de regalos, moderación, persistencia y reapertura sin lectura del historial, además de regresiones del MVP |
| Migración local | Preferencias y comentarios de 0.1.0 cargan con valores predeterminados para los campos nuevos |
| Análisis Rust | `cargo clippy --all-targets -- -D warnings` OK |
| Compilación | `.app` 0.2.0 generada para Apple Silicon, aproximadamente 123 MiB |
| Ventana real | Opciones de nombre/regalos seleccionables; espectadores simulados visibles; regalo DEMO Rosa × 3 con estado Leído y cola terminada |
| LIVE real | Puente conectado a `josephestebandev`; eventos de espectadores actuales con valores 6 y 5 |
| Historial reenviado por TikTok | Una sonda de metadatos del conector recibió 6 CHAT antiguos, de aproximadamente 20–35 minutos antes; el puente nuevo no los entregó a la cola |

En total, **32 pruebas automatizadas** correctas. Los regalos se probaron con payloads proto v3 y con la demostración; no se compraron ni observaron regalos reales durante esta comprobación. En la prueba real de 30 segundos se recibieron espectadores, sin comentarios nuevos entregados por el puente. La sonda de metadatos confirmó que TikTok reenvía comentarios anteriores incluso con `processInitialData: false`; sus horas originales quedan fuera del límite nuevo. No hubo un comentario recién enviado durante esa comprobación, por lo que no acredita nueva lectura de comentarios en producción. La recepción de comentarios reales de 0.1.0 está documentada abajo.

La protección contra historial ahora conserva la fecha de TikTok y descarta envolturas marcadas `isHistory`, eventos anteriores al inicio de la conexión y eventos sin hora válida. La biblioteca se configura con `processInitialData: false`. Una reconexión establece otro límite temporal. Con precisión de segundos se descarta también el segundo de conexión para evitar una lectura antigua (pueden perderse menos de un segundo de mensajes nuevos). Rust añade una segunda comprobación temporal y deduplicación de IDs; limpiar la vista no permite un replay.

Al conectar otra sesión se detiene la lectura anterior, se vacía la cola y se reinician comentarios y métricas de sesión. Al abrir la aplicación solo se recuperan las preferencias; consultar el historial tampoco encola sus comentarios. Los espectadores se muestran como `—` hasta recibir un dato actual y tras desconectarse. Las rachas de regalos se cuentan/anuncian una sola vez, al terminar; los regalos de distintos envíos siguen siendo independientes aunque sean del mismo usuario. Ambas preferencias nuevas son persistentes y están desactivadas por defecto.

## Resultados de la validación inicial (0.1.0)

| Comprobación | Resultado |
| --- | --- |
| TypeScript y build de Vite 8.3.4 | OK |
| Vitest 5.0.3 | 1 prueba OK: respuestas IPC antiguas no sobrescriben estados nuevos |
| Pruebas Node del adaptador | 4 OK: contrato antiguo, usuario inválido, contrato proto v3 actual y contenido desconocido |
| `cargo test` | 14 pruebas OK: filtros, vacíos/Unicode, deduplicación, FIFO, capacidad, caducidad, cancelación de demo, controles, modo manual, tokens de voz, errores nativos, historial y SQLite al reabrir |
| `cargo clippy --all-targets -- -D warnings` | OK, sin advertencias |
| `npm audit` después de actualizar las herramientas | 0 vulnerabilidades |
| Helper Swift con SDK de Command Line Tools | Compilado para arm64/macOS 13+ |
| `.app` Tauri release | Generada, aproximadamente 123 MiB; incluye Node y helper nativo |
| Apertura de `.app` por Launch Services mediante la herramienta de aplicaciones | OK |
| Voces instaladas | 180 voces listadas por AVSpeechSynthesizer en este Mac |
| Demo en la ventana real | 6 comentarios simulados completaron lectura nativa; contador 6 recibidos/6 leídos |
| Puente real con `josephestebandev` | Conectado; 6 eventos CHAT con texto no vacío en una prueba de 30 s |
| Flujo real en la `.app` | Usuarios reales, texto real y estado Leído visibles; también un repetido bloqueado correctamente |
| Preferencias al reabrir la aplicación | Último usuario y volumen conservados en la ventana |

## Correcciones encontradas durante la validación

La API actual de `tiktok-live-connector` 2.5.0 entrega eventos proto v3: `content` para el texto y `user.displayId` para el identificador del usuario. El primer adaptador usaba el campo antiguo `comment`, por lo que los eventos llegaban pero aparecían como `filtrado: vacío`. Se corrigió el mapeo, se añadió una prueba de regresión y se recompiló la aplicación. La prueba final comprueba longitud de texto y lectura en la ventana, además de contar eventos.

También se añadieron las capacidades `core:default` necesarias para suscripciones de eventos Tauri, normalización del usuario a minúsculas y manejo explícito de una cancelación inesperada de macOS.

## Limitaciones observadas

- **DMG:** tanto el empaquetador Tauri como `hdiutil` fallan en el entorno restringido con `Dispositivo no configurado`. `diskutil` informa que no puede utilizar DiskManagement. La `.app` sí se generó. Ejecuta `npm run tauri build` desde una Terminal normal para intentar generar el `.dmg`.
- **Prueba nativa desde el entorno restringido:** `tests/native-smoke.mjs` llegó a `started`, pero macOS canceló la lectura con `sandbox_extension_issue_file ... Operation not permitted`. Esto no ocurre al abrir la `.app` por Launch Services: allí se observaron finalizaciones reales y estados Leído en demo y LIVE. El script está disponible para ejecutarlo desde una Terminal normal y usa volumen 0.
- **Distribución:** no hay firma Developer ID ni notarización. El runtime incluido es Apple Silicon, no universal.
- **Conector:** recepción real verificada en esta sesión, no una garantía de disponibilidad futura. El protocolo no oficial y el servicio externo de firma pueden cambiar; no hubo fallback a simulación.
- **Reconexión y fin de LIVE:** implementados y revisados; no se provocó el fin de la transmisión del usuario ni una caída de su red para validarlos en producción.
- **Progreso:** no hay porcentaje artificial. Las ondas muestran actividad y la cola avanza con el evento real de finalización.

## Repetir las comprobaciones

Instala Node/Rust siguiendo el README y, si utilizas las Command Line Tools, configura `DEVELOPER_DIR=/Library/Developer/CommandLineTools`.

```sh
npm ci
npm run check
npm run prepare:native
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri build -- --bundles app
# Terminal normal, prueba de TTS real sin audio audible:
node tests/native-smoke.mjs
# Requiere que el usuario esté transmitiendo; solo lee comentarios:
node tests/live-probe.mjs josephestebandev 30 --bundled
```

La prueba LIVE guarda solo identificadores, procedencia y longitud de los textos en `work/live-probe.json`; no persiste cuerpos de comentarios ni credenciales.

## Preparación para la publicación en GitHub

Se repitió la instalación y validación desde una copia limpia de los archivos preparados para el repositorio, sin `node_modules`, recursos ejecutables ni herramientas locales incluidas en esa copia. `npm ci`, `npm run check`, `npm run prepare:native` y `cargo test --locked` finalizaron correctamente: 19 pruebas y compilación de frontend/helper Swift. Se utilizaron las herramientas de Node, Rust y Apple instaladas en el equipo.

Se añadió licencia MIT, instrucciones portables de compilación y lockfiles. Git se inicializó correctamente para la publicación. Las herramientas locales, bases de datos, binarios, registros y variables de entorno están excluidos mediante `.gitignore`.
