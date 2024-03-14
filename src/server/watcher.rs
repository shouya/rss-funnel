use std::path::{Path, PathBuf};

use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::error;

use crate::util::{ConfigError, Result};

pub struct Watcher {
  path: PathBuf,
  watcher: Option<notify::RecommendedWatcher>,
  rx: Option<Receiver<()>>,
  tx: Sender<()>,
}

impl Watcher {
  pub fn new(path: &Path) -> Result<Self> {
    let (tx, rx) = mpsc::channel(1);

    // sometimes the editor may touch the file multiple times in quick
    // succession when saving, so we debounce the events
    let rx = debounce(std::time::Duration::from_millis(500), rx);

    Ok(Self {
      path: path.to_owned(),
      watcher: None,
      rx: Some(rx),
      tx,
    })
  }

  pub fn take_change_alert(&mut self) -> Option<Receiver<()>> {
    self.rx.take()
  }

  pub fn setup(&mut self) -> Result<()> {
    use notify::{Event, RecursiveMode, Watcher};

    let tx = self.tx.clone();
    let event_handler = move |event: Result<Event, notify::Error>| match event {
      Ok(event) if event.kind.is_modify() => {
        tx.blocking_send(()).unwrap();
      }
      Ok(e) => {
        dbg!(e);
      }
      Err(_) => {
        error!("file watcher error: {:?}", event);
      }
    };

    let mut watcher =
      notify::recommended_watcher(event_handler).map_err(|e| {
        ConfigError::Message(format!("failed to create file watcher: {:?}", e))
      })?;

    watcher
      .watch(&self.path, RecursiveMode::NonRecursive)
      .map_err(|e| {
        ConfigError::Message(format!("failed to watch file: {:?}", e))
      })?;

    Ok(())
  }
}

fn debounce<T: Send + 'static>(
  duration: std::time::Duration,
  mut rx: Receiver<T>,
) -> Receiver<T> {
  let (debounced_tx, debounced_rx) = mpsc::channel(1);
  tokio::task::spawn(async move {
    let mut last = None;
    loop {
      tokio::select! {
        val = rx.recv() => {
          last = val;
        }
        _ = tokio::time::sleep(duration) => {
          if let Some(val) = last.take() {
            debounced_tx.send(val).await.unwrap();
          }
        }
      }
    }
  });
  debounced_rx
}
