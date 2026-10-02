//! Cancellable source work shared by one-shot and paginated sources.
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::Duration,
};

use crate::source::Cancellation;
use anyhow::Result;

pub struct Worker<T> {
    receiver: Receiver<Result<T>>,
    cancellation: Cancellation,
    handle: Option<thread::JoinHandle<()>>,
}

impl<T: Send + 'static> Worker<T> {
    pub fn spawn(task: impl FnOnce(&Cancellation) -> Result<T> + Send + 'static) -> Self {
        let cancellation = Cancellation::default();
        let flag = cancellation.clone();
        let (sender, receiver) = mpsc::channel();
        let handle = thread::spawn(move || {
            let _ = sender.send(task(&flag));
        });
        Self {
            receiver,
            cancellation,
            handle: Some(handle),
        }
    }

    pub fn try_recv(&self) -> std::result::Result<Result<T>, TryRecvError> {
        self.receiver.try_recv()
    }

    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> std::result::Result<Result<T>, mpsc::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }

    /// The worker thread still shuts down and reaps its child processes. Dropping
    /// its join handle avoids waiting for that shutdown during a filter change.
    pub fn cancel_in_background(mut self) {
        self.cancellation.cancel();
        self.handle.take();
    }
}

impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
