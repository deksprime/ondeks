use super::{Buffer, Sample};
use std::f32::consts::PI;

/// A pair of buffers representing stereo audio.
#[derive(Clone, Debug)]
pub struct StereoBuffer {
    left: Buffer,
    right: Buffer,
}

impl StereoBuffer {
    /// Create a stereo buffer filled with silence.
    pub fn allocate(size: usize) -> Self {
        Self {
            left: Buffer::allocate(size),
            right: Buffer::allocate(size),
        }
    }

    /// Create from separate left and right buffers.
    /// 
    /// # Panics
    /// Panics if sizes don't match.
    pub fn from_channels(left: Buffer, right: Buffer) -> Self {
        assert_eq!(left.len(), right.len(), "Channel size mismatch");
        Self { left, right }
    }

    /// Number of samples per channel.
    #[inline]
    pub fn len(&self) -> usize {
        self.left.len()
    }

    /// Whether the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.left.is_empty()
    }

    /// Get reference to left channel.
    #[inline]
    pub fn left(&self) -> &Buffer {
        &self.left
    }

    /// Get mutable reference to left channel.
    #[inline]
    pub fn left_mut(&mut self) -> &mut Buffer {
        &mut self.left
    }

    /// Get reference to right channel.
    #[inline]
    pub fn right(&self) -> &Buffer {
        &self.right
    }

    /// Get mutable reference to right channel.
    #[inline]
    pub fn right_mut(&mut self) -> &mut Buffer {
        &mut self.right
    }

    /// Fill both channels with silence.
    pub fn silence(&mut self) {
        self.left.silence();
        self.right.silence();
    }

    /// Apply gain to both channels.
    pub fn apply_gain(&mut self, gain: Sample) {
        self.left.apply_gain(gain);
        self.right.apply_gain(gain);
    }

    /// Apply stereo panning using constant-power law.
    /// 
    /// - pan = -1.0: full left
    /// - pan = 0.0: center
    /// - pan = 1.0: full right
    pub fn apply_pan(&mut self, pan: f32) {
        let pan = pan.clamp(-1.0, 1.0);
        // Constant power panning
        let angle = (pan + 1.0) * PI / 4.0; // 0 to PI/2
        let left_gain = angle.cos();
        let right_gain = angle.sin();
        
        self.left.apply_gain(left_gain);
        self.right.apply_gain(right_gain);
    }

    /// Copy from another stereo buffer.
    pub fn copy_from(&mut self, other: &StereoBuffer) {
        self.left.copy_from(&other.left);
        self.right.copy_from(&other.right);
    }

    /// Mix (add) from another stereo buffer.
    pub fn add_from(&mut self, other: &StereoBuffer) {
        self.left.add_from(&other.left);
        self.right.add_from(&other.right);
    }

    /// Convert interleaved samples [L, R, L, R, ...] to stereo buffer.
    pub fn from_interleaved(samples: &[Sample]) -> Self {
        assert!(samples.len() % 2 == 0, "Interleaved samples must have even length");
        let frame_count = samples.len() / 2;
        
        let mut left = Vec::with_capacity(frame_count);
        let mut right = Vec::with_capacity(frame_count);
        
        for chunk in samples.chunks(2) {
            left.push(chunk[0]);
            right.push(chunk[1]);
        }
        
        Self {
            left: Buffer::from_samples(left),
            right: Buffer::from_samples(right),
        }
    }

    /// Convert to interleaved samples [L, R, L, R, ...].
    pub fn to_interleaved(&self) -> Vec<Sample> {
        let mut result = Vec::with_capacity(self.len() * 2);
        for i in 0..self.len() {
            result.push(self.left[i]);
            result.push(self.right[i]);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_creates_silent_stereo() {
        let buf = StereoBuffer::allocate(256);
        assert_eq!(buf.len(), 256);
        assert!(buf.left().as_slice().iter().all(|&s| s == 0.0));
        assert!(buf.right().as_slice().iter().all(|&s| s == 0.0));
    }

    #[test]
    fn from_channels_works() {
        let left = Buffer::from_samples(vec![1.0, 2.0]);
        let right = Buffer::from_samples(vec![3.0, 4.0]);
        let stereo = StereoBuffer::from_channels(left, right);
        assert_eq!(stereo.left()[0], 1.0);
        assert_eq!(stereo.right()[0], 3.0);
    }

    #[test]
    #[should_panic(expected = "Channel size mismatch")]
    fn from_channels_panics_on_mismatch() {
        let left = Buffer::allocate(100);
        let right = Buffer::allocate(200);
        StereoBuffer::from_channels(left, right);
    }

    #[test]
    fn pan_center_maintains_balance() {
        let mut buf = StereoBuffer::allocate(4);
        buf.left_mut().fill(1.0);
        buf.right_mut().fill(1.0);
        buf.apply_pan(0.0);
        
        // At center, both channels should have equal gain (~0.707)
        let diff = (buf.left()[0] - buf.right()[0]).abs();
        assert!(diff < 0.01, "Center pan should be balanced");
    }

    #[test]
    fn pan_full_left() {
        let mut buf = StereoBuffer::allocate(4);
        buf.left_mut().fill(1.0);
        buf.right_mut().fill(1.0);
        buf.apply_pan(-1.0);
        
        // Full left: right channel should be ~0
        assert!(buf.right()[0].abs() < 0.01);
        // Left channel should be ~1
        assert!((buf.left()[0] - 1.0).abs() < 0.01);
    }

    #[test]
    fn pan_full_right() {
        let mut buf = StereoBuffer::allocate(4);
        buf.left_mut().fill(1.0);
        buf.right_mut().fill(1.0);
        buf.apply_pan(1.0);
        
        // Full right: left channel should be ~0
        assert!(buf.left()[0].abs() < 0.01);
        // Right channel should be ~1
        assert!((buf.right()[0] - 1.0).abs() < 0.01);
    }

    #[test]
    fn interleaved_roundtrip() {
        let original = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        let stereo = StereoBuffer::from_interleaved(&original);
        let result = stereo.to_interleaved();
        
        for (a, b) in original.iter().zip(result.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn from_interleaved_splits_correctly() {
        let interleaved = vec![1.0, 2.0, 3.0, 4.0];
        let stereo = StereoBuffer::from_interleaved(&interleaved);
        assert_eq!(stereo.left().as_slice(), &[1.0, 3.0]);
        assert_eq!(stereo.right().as_slice(), &[2.0, 4.0]);
    }

    #[test]
    fn add_from_mixes_stereo() {
        let mut a = StereoBuffer::allocate(2);
        a.left_mut().fill(0.3);
        a.right_mut().fill(0.4);
        
        let mut b = StereoBuffer::allocate(2);
        b.left_mut().fill(0.2);
        b.right_mut().fill(0.1);
        
        a.add_from(&b);
        
        assert!((a.left()[0] - 0.5).abs() < 1e-6);
        assert!((a.right()[0] - 0.5).abs() < 1e-6);
    }
}
