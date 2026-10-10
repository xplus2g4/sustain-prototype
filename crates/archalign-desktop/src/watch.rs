//! The watcher subscription (FR-WAT-01, ADR 0017): one `RepoWatcher` per open repository,
//! started when the repo opens and dropped when it closes or another one opens.

use std::hash::{Hash, Hasher};

use archalign_repo::{RepoWatcher, Sandbox, SelfWrites, WatchBatch};
use iced::Subscription;
use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};

use crate::app::Message;

/// Identifies one watch. A new generation (another repo, or the same one reopened) or a
/// changed "show hidden files" restarts it.
#[derive(Clone)]
pub struct WatchKey {
    /// Repo generation, echoed in every message.
    pub generation: u64,
    /// The repository.
    pub sandbox: Sandbox,
    /// The app's own writes (FR-WAT-08).
    pub writes: SelfWrites,
    /// Tree option used by the filter.
    pub show_hidden: bool,
}

impl Hash for WatchKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.generation.hash(state);
        self.sandbox.root().hash(state);
        self.show_hidden.hash(state);
    }
}

/// The subscription for `key`.
pub fn subscription(key: WatchKey) -> Subscription<Message> {
    Subscription::run_with(key, stream)
}

/// The message stream behind [`subscription`]: `WatchStarted` or `WatchFailed`, then one
/// `Watch` per batch. Dropping it stops the watcher.
pub fn stream(key: &WatchKey) -> impl Stream<Item = Message> + use<> {
    let key = key.clone();
    iced::stream::channel(64, async move |mut out: mpsc::Sender<Message>| {
        let g = key.generation;
        let (tx, mut rx) = mpsc::unbounded::<WatchBatch>();
        match RepoWatcher::start(
            key.sandbox.clone(),
            key.writes.clone(),
            key.show_hidden,
            move |b| {
                let _ = tx.unbounded_send(b);
            },
        ) {
            Ok(watcher) => {
                let _ = out.send(Message::WatchStarted(g)).await;
                while let Some(batch) = rx.next().await {
                    if out.send(Message::Watch(g, batch)).await.is_err() {
                        break;
                    }
                }
                drop(watcher);
            }
            Err(e) => {
                // FR-WAT-10: the status bar falls back to "Manual refresh only".
                let _ = out.send(Message::WatchFailed(g, e.to_string())).await;
                std::future::pending::<()>().await;
            }
        }
    })
}
