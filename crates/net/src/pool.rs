use crate::error::NetError;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::task::JoinSet;

/// Run async work over `items` with at most `limit` in flight, returning the
/// results in input order. The worker receives the item and its index.
/// A panicking worker yields `NetError::Join`.
pub async fn pool<T, R, F, Fut>(items: Vec<T>, limit: usize, worker: F) -> Result<Vec<R>, NetError>
where
    T: Send + 'static,
    R: Send + 'static,
    F: Fn(T, usize) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
{
    let total = items.len();
    let slots: Arc<Vec<Mutex<Option<T>>>> =
        Arc::new(items.into_iter().map(|t| Mutex::new(Some(t))).collect());
    let cursor = Arc::new(AtomicUsize::new(0));
    let worker = Arc::new(worker);
    let mut set = JoinSet::new();
    for _ in 0..limit.max(1).min(total) {
        let (slots, cursor, worker) = (slots.clone(), cursor.clone(), worker.clone());
        set.spawn(async move {
            let mut out: Vec<(usize, R)> = Vec::new();
            loop {
                let i = cursor.fetch_add(1, Ordering::SeqCst);
                if i >= slots.len() {
                    return out;
                }
                let item = slots[i].lock().unwrap_or_else(|e| e.into_inner()).take();
                if let Some(item) = item {
                    out.push((i, worker(item, i).await));
                }
            }
        });
    }
    let mut results: Vec<Option<R>> = (0..total).map(|_| None).collect();
    while let Some(joined) = set.join_next().await {
        let part = joined.map_err(|e| NetError::Join(e.to_string()))?;
        for (i, r) in part {
            results[i] = Some(r);
        }
    }
    results
        .into_iter()
        .map(|r| r.ok_or_else(|| NetError::Join("missing result".into())))
        .collect()
}
