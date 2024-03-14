use std::path::{Path, PathBuf};

use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::error;

use crate::util::{ConfigError, Result};

pub struct Watcher {
  path: PathBuf,
  watcher: Option<notify::RecommendedWatcher>,
  rx: Option<Receiver<()>>,
  tx: Sender<()>,
  reload_tx: Sender<()>,
  reload_rx: Receiver<()>,
}

impl Watcher {
  pub fn new(path: &Path) -> Result<Self> {
    let (tx, rx) = mpsc::channel(1);
    let (reload_tx, reload_rx) = mpsc::channel(1);

    // sometimes the editor may touch the file multiple times in quick
    // succession when saving, so we debounce the events
    let rx = debounce(std::time::Duration::from_millis(500), rx);

    Ok(Self {
      path: path.to_owned(),
      watcher: None,
      reload_tx,
      reload_rx,
      rx: Some(rx),
      tx,
    })
  }

  pub fn take_change_alert(&mut self) -> Option<Receiver<()>> {
    self.rx.take()
  }

  pub async fn run(mut self) -> Result<()> {
    self.setup()?;

    loop {
      self.reload_rx.recv().await.unwrap();
      self.setup()?;
      // the file is re-created, trigger a reload
      self.tx.send(()).await.unwrap();
    }
  }

  fn setup(&mut self) -> Result<()> {
    use notify::{
      event::{ModifyKind, RemoveKind},
      Event, EventKind, RecursiveMode, Watcher,
    };

    let tx = self.tx.clone();
    let reload_tx = self.reload_tx.clone();
    let event_handler = move |event: Result<Event, notify::Error>| match event {
      Ok(Event {
        kind: EventKind::Modify(ModifyKind::Data(_)),
        ..
      }) => {
        tx.blocking_send(()).unwrap();
      }
      Ok(Event {
        kind: EventKind::Remove(RemoveKind::File),
        ..
      }) => {
        // Captures vim's backupcopy=yes behavior. The file is likely
        // renamed and deleted, try monitor the same file name again.
        reload_tx.blocking_send(()).unwrap();
      }
      Ok(_event) => {}
      Err(_) => {
        error!("file watcher error: {:?}", event);
      }
    };

    let mut watcher =
      notify::recommended_watcher(event_handler).map_err(|e| {
        ConfigError::Message(format!("failed to create file watcher: {:?}", e))
      })?;

    // if the file does not exist, simply wait for it to be created
    while !self.path.exists() {
      error!(
        "{} does not exist, waiting for it to be created",
        self.path.display()
      );
      std::thread::sleep(std::time::Duration::from_secs(5));
    }

    watcher
      .watch(&self.path, RecursiveMode::NonRecursive)
      .map_err(|e| {
        ConfigError::Message(format!("failed to watch file: {:?}", e))
      })?;

    self.watcher.replace(watcher);

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
