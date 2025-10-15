#![cfg(not(target_family = "wasm"))]

use async_channel::{Receiver, Sender};

#[derive(Debug)]
pub struct WorkerManager {
  free_workers_sender: Sender<u16>,
  free_workers_receiver: Receiver<u16>,
  worker_count: u16,
}

#[cfg(not(target_family = "wasm"))]
impl WorkerManager {
  pub fn new(worker_count: u16) -> Self {
    let (sender, receiver) = async_channel::unbounded::<u16>();

    (0..worker_count).for_each(|value| {
      sender
        .send_blocking(value)
        .expect("WorkerManager: failed to initialize worker pool - channel closed during setup");
    });

    Self { free_workers_sender: sender, free_workers_receiver: receiver, worker_count }
  }

  pub async fn acquire(&self) -> WorkerSemaphorePermit {
    let worker_index = self.free_workers_receiver.recv().await.unwrap();

    WorkerSemaphorePermit { worker_index, sender: self.free_workers_sender.clone() }
  }

  pub async fn acquire_all(&self) -> WorkerAllSemaphorePermit {
    for _ in 0..self.worker_count {
      self.free_workers_receiver.recv().await.unwrap();
    }

    WorkerAllSemaphorePermit {
      worker_count: self.worker_count,
      sender: self.free_workers_sender.clone(),
    }
  }
}

#[cfg(not(target_family = "wasm"))]
pub struct WorkerSemaphorePermit {
  worker_index: u16,
  sender: Sender<u16>,
}

#[cfg(not(target_family = "wasm"))]
impl WorkerSemaphorePermit {
  pub fn worker_index(&self) -> u16 {
    self.worker_index
  }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for WorkerSemaphorePermit {
  fn drop(&mut self) {
    let worker_index = self.worker_index;
    self.sender.send_blocking(worker_index).expect(
      "WorkerManager: failed to return worker to pool - channel closed while worker was active",
    );
  }
}

#[cfg(not(target_family = "wasm"))]
pub struct WorkerAllSemaphorePermit {
  worker_count: u16,
  sender: Sender<u16>,
}

#[cfg(not(target_family = "wasm"))]
impl Drop for WorkerAllSemaphorePermit {
  fn drop(&mut self) {
    let worker_count = self.worker_count;
    (0..worker_count).for_each(|value| {
      self
        .sender
        .send_blocking(value)
        .expect("WorkerManager: failed to return workers to pool during bulk release - channel closed unexpectedly");
    });
  }
}
