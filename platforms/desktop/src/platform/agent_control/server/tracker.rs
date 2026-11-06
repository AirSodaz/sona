use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::watch;

#[derive(Clone)]
pub struct ActiveConnectionTracker {
    pub(crate) count: Arc<AtomicUsize>,
    pub(crate) disconnect_tx: Arc<watch::Sender<u64>>,
    pub(crate) disconnect_rx: watch::Receiver<u64>,
}

impl Default for ActiveConnectionTracker {
    fn default() -> Self {
        let (disconnect_tx, disconnect_rx) = watch::channel(0);
        Self {
            count: Arc::new(AtomicUsize::new(0)),
            disconnect_tx: Arc::new(disconnect_tx),
            disconnect_rx,
        }
    }
}

impl ActiveConnectionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_connected(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn on_disconnected(&self) {
        let mut current = self.count.load(Ordering::SeqCst);
        loop {
            if current == 0 {
                break;
            }
            match self.count.compare_exchange_weak(
                current,
                current - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    pub fn active_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    pub fn disconnect_all(&self) {
        self.disconnect_tx.send_modify(|v| *v += 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_increments_and_decrements_cleanly() {
        let tracker = ActiveConnectionTracker::new();
        assert_eq!(tracker.active_count(), 0);

        tracker.on_connected();
        tracker.on_connected();
        assert_eq!(tracker.active_count(), 2);

        tracker.on_disconnected();
        assert_eq!(tracker.active_count(), 1);

        tracker.disconnect_all();
        assert_eq!(*tracker.disconnect_rx.borrow(), 1);
    }
}
