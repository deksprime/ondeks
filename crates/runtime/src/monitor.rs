use std::time::{Duration, Instant};
use std::collections::VecDeque;

/// Performance statistics.
#[derive(Debug, Clone, Default)]
pub struct PerformanceStats {
    /// CPU usage of audio thread (0.0 - 1.0).
    pub audio_cpu: f32,
    /// Average callback time in microseconds.
    pub avg_callback_us: f32,
    /// Maximum callback time seen.
    pub max_callback_us: f32,
    /// Number of buffer underruns since last reset.
    pub underruns: u64,
    /// Current buffer fill level (0.0 - 1.0).
    pub buffer_fill: f32,
    /// Round-trip latency in milliseconds.
    pub latency_ms: f32,
}

/// Tracks audio performance metrics.
pub struct PerformanceMonitor {
    /// Recent callback durations for averaging.
    callback_times: VecDeque<Duration>,
    max_samples: usize,
    /// Start of current callback.
    callback_start: Option<Instant>,
    /// Total underruns.
    underruns: u64,
    /// Buffer size in samples.
    buffer_size: usize,
    /// Sample rate.
    sample_rate: u32,
}

impl PerformanceMonitor {
    pub fn new(buffer_size: usize, sample_rate: u32) -> Self {
        Self {
            callback_times: VecDeque::with_capacity(100),
            max_samples: 100,
            callback_start: None,
            underruns: 0,
            buffer_size,
            sample_rate,
        }
    }

    /// Call at the start of audio callback.
    pub fn begin_callback(&mut self) {
        self.callback_start = Some(Instant::now());
    }

    /// Call at the end of audio callback.
    pub fn end_callback(&mut self) {
        if let Some(start) = self.callback_start.take() {
            let duration = start.elapsed();
            
            if self.callback_times.len() >= self.max_samples {
                self.callback_times.pop_front();
            }
            self.callback_times.push_back(duration);
        }
    }

    /// Report a buffer underrun.
    pub fn report_underrun(&mut self) {
        self.underruns += 1;
    }

    /// Reset underrun counter.
    pub fn reset_underruns(&mut self) {
        self.underruns = 0;
    }

    /// Get current performance statistics.
    pub fn stats(&self) -> PerformanceStats {
        let (avg_us, max_us) = if self.callback_times.is_empty() {
            (0.0, 0.0)
        } else {
            let sum: Duration = self.callback_times.iter().sum();
            let avg = sum / self.callback_times.len() as u32;
            let max = self.callback_times.iter().max().copied().unwrap_or_default();
            (avg.as_micros() as f32, max.as_micros() as f32)
        };

        // Calculate CPU usage: time spent / time available
        let buffer_duration_us = self.buffer_size as f32 / self.sample_rate as f32 * 1_000_000.0;
        let cpu = avg_us / buffer_duration_us;

        // Calculate latency (output latency only, simplified)
        let latency_ms = self.buffer_size as f32 / self.sample_rate as f32 * 1000.0;

        PerformanceStats {
            audio_cpu: cpu.min(1.0),
            avg_callback_us: avg_us,
            max_callback_us: max_us,
            underruns: self.underruns,
            buffer_fill: 1.0, // Would need ring buffer monitoring for real value
            latency_ms,
        }
    }

    /// Check if we're at risk of underruns.
    pub fn is_at_risk(&self) -> bool {
        let stats = self.stats();
        stats.audio_cpu > 0.8 || stats.max_callback_us > (self.buffer_size as f32 / self.sample_rate as f32 * 1_000_000.0 * 0.9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn performance_monitor_basic() {
        let mut monitor = PerformanceMonitor::new(512, 44100);
        
        monitor.begin_callback();
        std::thread::sleep(Duration::from_micros(100));
        monitor.end_callback();
        
        let stats = monitor.stats();
        assert!(stats.avg_callback_us > 0.0);
    }

    #[test]
    fn underrun_tracking() {
        let mut monitor = PerformanceMonitor::new(512, 44100);
        
        assert_eq!(monitor.stats().underruns, 0);
        
        monitor.report_underrun();
        assert_eq!(monitor.stats().underruns, 1);
        
        monitor.report_underrun();
        assert_eq!(monitor.stats().underruns, 2);
        
        monitor.reset_underruns();
        assert_eq!(monitor.stats().underruns, 0);
    }

    #[test]
    fn latency_calculation() {
        let monitor = PerformanceMonitor::new(1024, 48000);
        let stats = monitor.stats();
        
        // 1024 samples at 48000 Hz = ~21.33 ms
        assert!((stats.latency_ms - 21.33).abs() < 0.1);
    }
}
