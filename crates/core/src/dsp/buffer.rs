use super::sample::Sample;

/// A fixed-size buffer of audio samples for a single channel.
///
/// Buffers are the fundamental unit of audio data passed between nodes
/// in the audio graph. They are always a fixed size determined by the
/// runtime's buffer size configuration.
#[derive(Clone, Debug)]
pub struct Buffer {
    samples: Vec<Sample>,
}

impl Buffer {
    /// Create a new buffer filled with silence.
    pub fn allocate(size: usize) -> Self {
        Self {
            samples: vec![0.0; size],
        }
    }

    /// Create a buffer from existing samples.
    pub fn from_samples(samples: Vec<Sample>) -> Self {
        Self { samples }
    }

    /// Number of samples in this buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Fill the entire buffer with a single value.
    pub fn fill(&mut self, value: Sample) {
        self.samples.fill(value);
    }

    /// Fill the entire buffer with silence (zeros).
    pub fn silence(&mut self) {
        self.fill(0.0);
    }

    /// Get a reference to the underlying samples.
    #[inline]
    pub fn as_slice(&self) -> &[Sample] {
        &self.samples
    }

    /// Get a mutable reference to the underlying samples.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Sample] {
        &mut self.samples
    }

    /// Copy samples from another buffer.
    /// 
    /// # Panics
    /// Panics if sizes don't match.
    pub fn copy_from(&mut self, other: &Buffer) {
        assert_eq!(
            self.len(),
            other.len(),
            "Buffer size mismatch: {} vs {}",
            self.len(),
            other.len()
        );
        self.samples.copy_from_slice(&other.samples);
    }

    /// Add samples from another buffer (mixing).
    /// 
    /// # Panics
    /// Panics if sizes don't match.
    pub fn add_from(&mut self, other: &Buffer) {
        assert_eq!(self.len(), other.len(), "Buffer size mismatch");
        for (dst, src) in self.samples.iter_mut().zip(other.samples.iter()) {
            *dst += *src;
        }
    }

    /// Multiply all samples by a gain value.
    pub fn apply_gain(&mut self, gain: Sample) {
        for sample in &mut self.samples {
            *sample *= gain;
        }
    }

    /// Get the peak absolute value in the buffer.
    pub fn peak(&self) -> Sample {
        self.samples
            .iter()
            .map(|s| s.abs())
            .fold(0.0, f32::max)
    }

    /// Get the RMS (root mean square) level.
    pub fn rms(&self) -> Sample {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum_squares: f32 = self.samples.iter().map(|s| s * s).sum();
        (sum_squares / self.samples.len() as f32).sqrt()
    }
}

impl std::ops::Index<usize> for Buffer {
    type Output = Sample;

    #[inline]
    fn index(&self, index: usize) -> &Sample {
        &self.samples[index]
    }
}

impl std::ops::IndexMut<usize> for Buffer {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Sample {
        &mut self.samples[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_creates_silent_buffer() {
        let buf = Buffer::allocate(512);
        assert_eq!(buf.len(), 512);
        assert!(buf.as_slice().iter().all(|&s| s == 0.0));
    }

    #[test]
    fn from_samples_preserves_data() {
        let data = vec![0.1, 0.2, 0.3];
        let buf = Buffer::from_samples(data.clone());
        assert_eq!(buf.as_slice(), &data[..]);
    }

    #[test]
    fn fill_sets_all_samples() {
        let mut buf = Buffer::allocate(256);
        buf.fill(0.5);
        assert!(buf.as_slice().iter().all(|&s| s == 0.5));
    }

    #[test]
    fn silence_zeros_buffer() {
        let mut buf = Buffer::from_samples(vec![1.0, 2.0, 3.0]);
        buf.silence();
        assert!(buf.as_slice().iter().all(|&s| s == 0.0));
    }

    #[test]
    fn copy_from_duplicates_content() {
        let src = Buffer::from_samples(vec![0.1, 0.2, 0.3, 0.4]);
        let mut dst = Buffer::allocate(4);
        dst.copy_from(&src);
        assert_eq!(dst.as_slice(), src.as_slice());
    }

    #[test]
    fn add_from_mixes_buffers() {
        let mut a = Buffer::from_samples(vec![0.5, 0.5]);
        let b = Buffer::from_samples(vec![0.3, 0.2]);
        a.add_from(&b);
        assert!((a[0] - 0.8).abs() < 1e-6);
        assert!((a[1] - 0.7).abs() < 1e-6);
    }

    #[test]
    fn apply_gain_scales_samples() {
        let mut buf = Buffer::from_samples(vec![1.0, -1.0, 0.5]);
        buf.apply_gain(0.5);
        assert_eq!(buf.as_slice(), &[0.5, -0.5, 0.25]);
    }

    #[test]
    fn peak_finds_maximum_absolute() {
        let buf = Buffer::from_samples(vec![0.1, -0.9, 0.5, 0.3]);
        assert!((buf.peak() - 0.9).abs() < 1e-6);
    }

    #[test]
    fn peak_of_empty_is_zero() {
        let buf = Buffer::allocate(0);
        assert_eq!(buf.peak(), 0.0);
    }

    #[test]
    fn rms_calculates_correctly() {
        // RMS of [1, 1, 1, 1] = 1
        let buf = Buffer::from_samples(vec![1.0, 1.0, 1.0, 1.0]);
        assert!((buf.rms() - 1.0).abs() < 1e-6);

        // RMS of [1, -1, 1, -1] = 1
        let buf2 = Buffer::from_samples(vec![1.0, -1.0, 1.0, -1.0]);
        assert!((buf2.rms() - 1.0).abs() < 1e-6);

        // RMS of silence = 0
        let buf3 = Buffer::allocate(100);
        assert_eq!(buf3.rms(), 0.0);
    }

    #[test]
    fn indexing_works() {
        let mut buf = Buffer::from_samples(vec![1.0, 2.0, 3.0]);
        assert_eq!(buf[1], 2.0);
        buf[1] = 5.0;
        assert_eq!(buf[1], 5.0);
    }

    #[test]
    #[should_panic(expected = "Buffer size mismatch")]
    fn copy_from_panics_on_size_mismatch() {
        let src = Buffer::allocate(256);
        let mut dst = Buffer::allocate(512);
        dst.copy_from(&src);
    }

    #[test]
    #[should_panic(expected = "Buffer size mismatch")]
    fn add_from_panics_on_size_mismatch() {
        let mut a = Buffer::allocate(256);
        let b = Buffer::allocate(512);
        a.add_from(&b);
    }
}
