//! Small, bounded snapshots; disk writes and pruning stay off the UI thread.
use super::Page;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender},
    },
    thread,
};

const MAX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_FILES: usize = 100;

fn value_bytes(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::String(text) => text.len().saturating_mul(6).saturating_add(2),
        serde_json::Value::Array(values) => values
            .iter()
            .map(value_bytes)
            .fold(2usize, usize::saturating_add)
            .saturating_add(values.len()),
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(key, value)| {
                key.len()
                    .saturating_mul(6)
                    .saturating_add(4)
                    .saturating_add(value_bytes(value))
            })
            .fold(2usize, usize::saturating_add),
        _ => 32,
    }
}

pub(super) fn fits_page(page: &Page) -> bool {
    let bytes = page
        .items
        .iter()
        .flat_map(|item| item.iter())
        .map(|(key, value)| {
            key.len()
                .saturating_mul(6)
                .saturating_add(4)
                .saturating_add(value_bytes(value))
        })
        .fold(1024usize, usize::saturating_add);
    bytes <= MAX_BYTES as usize
}

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Cached {
    pub fetched_ms: u64,
    pub page: Page,
}

impl Cached {
    pub fn fresh(&self, now: u64, ttl: u64) -> bool {
        ttl > 0 && now >= self.fetched_ms && now - self.fetched_ms < ttl
    }
}

pub(super) fn read(path: &Path) -> Option<Cached> {
    let file = fs::File::open(path).ok()?;
    let mut data = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut data).ok()?;
    if data.len() as u64 > MAX_BYTES {
        return None;
    }
    serde_json::from_slice(&data).ok()
}

pub(super) fn write(path: &Path, cache: &Cached) -> anyhow::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cache directory missing"))?;
    fs::create_dir_all(directory)?;
    let data = serde_json::to_vec(cache)?;
    if data.len() as u64 <= MAX_BYTES {
        crate::storage::atomic_write(path, &data, Some(0o600))?;
    }
    Ok(())
}

fn prune(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut files: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()?.to_str()? != "json" || !entry.file_type().ok()?.is_file() {
                return None;
            }
            Some((entry.metadata().ok()?.modified().ok()?, path))
        })
        .collect();
    files.sort_by_key(|(modified, _)| *modified);
    let excess = files.len().saturating_sub(MAX_FILES);
    for (_, path) in files.into_iter().take(excess) {
        let _ = fs::remove_file(path);
    }
}

pub(super) struct Writer {
    sender: Option<SyncSender<()>>,
    pending: Arc<Mutex<VecDeque<(PathBuf, Cached)>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Writer {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel::<()>(1);
        let pending = Arc::new(Mutex::new(VecDeque::<(PathBuf, Cached)>::new()));
        let queue = pending.clone();
        let handle = thread::spawn(move || {
            while receiver.recv().is_ok() {
                let batch = std::mem::take(&mut *queue.lock().expect("cache queue"));
                for (path, cache) in batch {
                    let _ = write(&path, &cache);
                    if let Some(directory) = path.parent() {
                        prune(directory);
                    }
                }
            }
        });
        Self {
            sender: Some(sender),
            pending,
            handle: Some(handle),
        }
    }

    pub fn submit(&self, path: PathBuf, cache: Cached) {
        // Coalesce by path rather than dropping the active source cache when a
        // filter-availability snapshot happens to arrive at the same time.
        let mut pending = self.pending.lock().expect("cache queue");
        pending.retain(|(existing, _)| existing != &path);
        if pending.len() >= 8 {
            pending.pop_front();
        }
        pending.push_back((path, cache));
        drop(pending);
        let _ = self
            .sender
            .as_ref()
            .expect("active cache writer")
            .try_send(());
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rem_006_cache_writes_flush_prune_and_use_private_permissions() {
        let directory = std::env::temp_dir().join(format!(
            "vellum-cache-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        for index in 0..MAX_FILES + 2 {
            fs::write(directory.join(format!("{index:016x}.json")), "{}").unwrap();
        }
        prune(&directory);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), MAX_FILES);
        let path = directory.join("ffffffffffffffff.json");
        let writer = Writer::new();
        writer.submit(
            path.clone(),
            Cached {
                fetched_ms: 100,
                page: Page::default(),
            },
        );
        drop(writer);
        assert_eq!(read(&path).unwrap().fetched_ms, 100);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), MAX_FILES);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let oversized = Cached {
            fetched_ms: 200,
            page: Page {
                items: vec![
                    serde_json::from_value(
                        serde_json::json!({"body": "x".repeat(MAX_BYTES as usize)}),
                    )
                    .unwrap(),
                ],
                next_cursor: None,
                filter_availability: None,
            },
        };
        write(&path, &oversized).unwrap();
        assert_eq!(read(&path).unwrap().fetched_ms, 100);
        fs::remove_dir_all(directory).unwrap();
    }
}
