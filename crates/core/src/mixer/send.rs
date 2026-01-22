use crate::ids::TrackId;
use crate::dsp::db_to_linear;

/// A send from one track to a return track.
#[derive(Debug, Clone)]
pub struct Send {
    pub source: TrackId,
    pub destination: TrackId,
    pub level_db: f32,
    pub pre_fader: bool,
    pub enabled: bool,
}

impl Send {
    pub fn new(source: TrackId, destination: TrackId) -> Self {
        Self {
            source,
            destination,
            level_db: -6.0,
            pre_fader: false,
            enabled: true,
        }
    }

    pub fn gain(&self) -> f32 {
        if self.enabled {
            db_to_linear(self.level_db)
        } else {
            0.0
        }
    }
}

/// Manages all sends in the mixer.
#[derive(Debug, Default)]
pub struct SendMatrix {
    sends: Vec<Send>,
}

impl SendMatrix {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a send.
    pub fn add_send(&mut self, send: Send) {
        self.sends.push(send);
    }

    /// Remove sends from a source track.
    pub fn remove_sends_from(&mut self, source: TrackId) {
        self.sends.retain(|s| s.source != source);
    }

    /// Get sends from a specific track.
    pub fn sends_from(&self, source: TrackId) -> impl Iterator<Item = &Send> {
        self.sends.iter().filter(move |s| s.source == source)
    }

    /// Get sends to a specific return track.
    pub fn sends_to(&self, destination: TrackId) -> impl Iterator<Item = &Send> {
        self.sends.iter().filter(move |s| s.destination == destination)
    }

    /// Get all sends.
    pub fn all(&self) -> &[Send] {
        &self.sends
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_send_defaults() {
        let source = TrackId::generate();
        let dest = TrackId::generate();
        let send = Send::new(source, dest);
        
        assert_eq!(send.source, source);
        assert_eq!(send.destination, dest);
        assert_eq!(send.level_db, -6.0);
        assert!(!send.pre_fader);
        assert!(send.enabled);
    }

    #[test]
    fn send_gain_enabled() {
        let source = TrackId::generate();
        let dest = TrackId::generate();
        let send = Send::new(source, dest);
        
        // -6dB should be approximately 0.5
        let gain = send.gain();
        assert!((gain - 0.5).abs() < 0.01);
    }

    #[test]
    fn send_gain_disabled() {
        let source = TrackId::generate();
        let dest = TrackId::generate();
        let mut send = Send::new(source, dest);
        send.enabled = false;
        
        assert_eq!(send.gain(), 0.0);
    }

    #[test]
    fn send_matrix_add_and_retrieve() {
        let mut matrix = SendMatrix::new();
        let source1 = TrackId::generate();
        let source2 = TrackId::generate();
        let dest = TrackId::generate();
        
        let send1 = Send::new(source1, dest);
        let send2 = Send::new(source2, dest);
        
        matrix.add_send(send1);
        matrix.add_send(send2);
        
        assert_eq!(matrix.all().len(), 2);
    }

    #[test]
    fn send_matrix_sends_from() {
        let mut matrix = SendMatrix::new();
        let source1 = TrackId::generate();
        let source2 = TrackId::generate();
        let dest = TrackId::generate();
        
        matrix.add_send(Send::new(source1, dest));
        matrix.add_send(Send::new(source2, dest));
        matrix.add_send(Send::new(source1, dest));
        
        let sends: Vec<_> = matrix.sends_from(source1).collect();
        assert_eq!(sends.len(), 2);
        
        let sends: Vec<_> = matrix.sends_from(source2).collect();
        assert_eq!(sends.len(), 1);
    }

    #[test]
    fn send_matrix_sends_to() {
        let mut matrix = SendMatrix::new();
        let source1 = TrackId::generate();
        let source2 = TrackId::generate();
        let dest1 = TrackId::generate();
        let dest2 = TrackId::generate();
        
        matrix.add_send(Send::new(source1, dest1));
        matrix.add_send(Send::new(source2, dest1));
        matrix.add_send(Send::new(source1, dest2));
        
        let sends: Vec<_> = matrix.sends_to(dest1).collect();
        assert_eq!(sends.len(), 2);
        
        let sends: Vec<_> = matrix.sends_to(dest2).collect();
        assert_eq!(sends.len(), 1);
    }

    #[test]
    fn send_matrix_remove_sends_from() {
        let mut matrix = SendMatrix::new();
        let source1 = TrackId::generate();
        let source2 = TrackId::generate();
        let dest = TrackId::generate();
        
        matrix.add_send(Send::new(source1, dest));
        matrix.add_send(Send::new(source2, dest));
        matrix.add_send(Send::new(source1, dest));
        
        assert_eq!(matrix.all().len(), 3);
        
        matrix.remove_sends_from(source1);
        
        assert_eq!(matrix.all().len(), 1);
        assert_eq!(matrix.all()[0].source, source2);
    }
}
