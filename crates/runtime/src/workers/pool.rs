use std::thread::{self, JoinHandle};
use crossbeam_channel::{bounded, Sender, Receiver};

/// A task to be executed by a worker.
pub trait Task: Send + 'static {
    type Output: Send + 'static;
    fn execute(self) -> Self::Output;
}

/// A handle to a pending task.
pub struct TaskHandle<T> {
    receiver: Receiver<T>,
}

impl<T> TaskHandle<T> {
    /// Check if the task is complete (non-blocking).
    pub fn try_get(&self) -> Option<T> {
        self.receiver.try_recv().ok()
    }

    /// Wait for the task to complete (blocking).
    pub fn wait(self) -> T {
        self.receiver.recv().expect("Worker thread died")
    }

    /// Check if task is done without consuming the result.
    pub fn is_done(&self) -> bool {
        !self.receiver.is_empty()
    }
}

/// Internal message type.
enum WorkerMessage {
    Execute(Box<dyn FnOnce() + Send>),
    Shutdown,
}

/// A pool of worker threads.
pub struct WorkerPool {
    sender: Sender<WorkerMessage>,
    threads: Vec<JoinHandle<()>>,
}

impl WorkerPool {
    /// Create a new worker pool with the specified number of threads.
    pub fn new(num_threads: usize) -> Self {
        let (sender, receiver) = bounded::<WorkerMessage>(256);
        
        let threads: Vec<_> = (0..num_threads)
            .map(|i| {
                let receiver = receiver.clone();
                thread::Builder::new()
                    .name(format!("deksound-worker-{}", i))
                    .spawn(move || {
                        while let Ok(msg) = receiver.recv() {
                            match msg {
                                WorkerMessage::Execute(task) => task(),
                                WorkerMessage::Shutdown => break,
                            }
                        }
                    })
                    .expect("Failed to spawn worker thread")
            })
            .collect();

        Self { sender, threads }
    }

    /// Submit a task and get a handle to wait for the result.
    pub fn submit<T, F>(&self, task: F) -> TaskHandle<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let (result_sender, result_receiver) = bounded(1);
        
        let wrapped = move || {
            let result = task();
            let _ = result_sender.send(result);
        };

        self.sender.send(WorkerMessage::Execute(Box::new(wrapped)))
            .expect("Worker pool is shut down");

        TaskHandle {
            receiver: result_receiver,
        }
    }

    /// Get the number of threads in the pool.
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        // Send shutdown to all workers
        for _ in 0..self.threads.len() {
            let _ = self.sender.send(WorkerMessage::Shutdown);
        }
        
        // Wait for all threads to finish
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

/// Common async tasks.
pub mod tasks {
    use super::*;
    use std::path::PathBuf;

    /// Load an audio file in the background.
    pub fn load_audio_file(pool: &WorkerPool, path: PathBuf) -> TaskHandle<Result<LoadedAudio, String>> {
        pool.submit(move || {
            match hound::WavReader::open(&path) {
                Ok(reader) => {
                    let spec = reader.spec();
                    let samples: Vec<f32> = match spec.sample_format {
                        hound::SampleFormat::Int => {
                            reader.into_samples::<i16>()
                                .filter_map(|s| s.ok())
                                .map(|s| s as f32 / i16::MAX as f32)
                                .collect()
                        }
                        hound::SampleFormat::Float => {
                            reader.into_samples::<f32>()
                                .filter_map(|s| s.ok())
                                .collect()
                        }
                    };

                    Ok(LoadedAudio {
                        samples,
                        sample_rate: spec.sample_rate,
                        channels: spec.channels as usize,
                    })
                }
                Err(e) => Err(e.to_string()),
            }
        })
    }

    /// Result of loading an audio file.
    #[derive(Debug)]
    pub struct LoadedAudio {
        pub samples: Vec<f32>,
        pub sample_rate: u32,
        pub channels: usize,
    }

    /// Generate waveform display data.
    pub fn generate_waveform(
        pool: &WorkerPool,
        samples: Vec<f32>,
        channels: usize,
        target_width: usize,
    ) -> TaskHandle<WaveformData> {
        pool.submit(move || {
            let frames = samples.len() / channels;
            let samples_per_pixel = frames / target_width.max(1);
            
            let mut peaks = Vec::with_capacity(target_width);
            
            for pixel in 0..target_width {
                let start = pixel * samples_per_pixel * channels;
                let end = ((pixel + 1) * samples_per_pixel * channels).min(samples.len());
                
                let mut min = 0.0f32;
                let mut max = 0.0f32;
                
                for i in (start..end).step_by(channels) {
                    let sample = samples.get(i).copied().unwrap_or(0.0);
                    min = min.min(sample);
                    max = max.max(sample);
                }
                
                peaks.push((min, max));
            }

            WaveformData { peaks }
        })
    }

    /// Waveform display data.
    #[derive(Debug)]
    pub struct WaveformData {
        /// (min, max) pairs for each display column.
        pub peaks: Vec<(f32, f32)>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_pool_basic() {
        let pool = WorkerPool::new(2);
        assert_eq!(pool.thread_count(), 2);
        
        let handle = pool.submit(|| {
            42
        });
        
        let result = handle.wait();
        assert_eq!(result, 42);
    }

    #[test]
    fn multiple_tasks() {
        let pool = WorkerPool::new(4);
        
        let handles: Vec<_> = (0..10)
            .map(|i| pool.submit(move || i * 2))
            .collect();
        
        let results: Vec<_> = handles.into_iter()
            .map(|h| h.wait())
            .collect();
        
        assert_eq!(results, vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);
    }

    #[test]
    fn task_handle_is_done() {
        let pool = WorkerPool::new(1);
        let handle = pool.submit(|| {
            std::thread::sleep(std::time::Duration::from_millis(10));
            42
        });
        
        // Should eventually become done
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(handle.is_done());
    }
}
