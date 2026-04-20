//! Event queue for receiving events from the audio thread.

use crossbeam_channel::{bounded, Sender, Receiver};
use ondeks_core::Event;
use ondeks_core::transport::BarBeatTick;

/// Events emitted by the audio engine.
#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    /// Core engine event.
    Core(Event),
    /// Audio underrun occurred.
    Underrun,
    /// Meter update (for UI VU meters).
    MeterUpdate { left: f32, right: f32 },
    /// Position update (sent periodically, not every buffer).
    PositionUpdate { 
        samples: u64, 
        beats: f64,
        bbt: BarBeatTick,
    },
    /// Transport state changed.
    TransportStateChanged { is_playing: bool },
}

/// Queue for receiving events from the audio thread.
pub struct EventQueue {
    sender: Sender<RuntimeEvent>,
    receiver: Receiver<RuntimeEvent>,
}

impl EventQueue {
    /// Create a new event queue with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        Self { sender, receiver }
    }

    /// Get the sender (for audio thread).
    pub fn sender(&self) -> EventSender {
        EventSender {
            inner: self.sender.clone(),
        }
    }

    /// Get the receiver (for non-audio threads).
    pub fn receiver(&self) -> EventReceiver {
        EventReceiver {
            inner: self.receiver.clone(),
        }
    }
}

/// Send side of the event queue (cloneable).
#[derive(Clone)]
pub struct EventSender {
    inner: Sender<RuntimeEvent>,
}

impl EventSender {
    /// Send an event. Non-blocking, drops if full.
    pub fn send(&self, event: RuntimeEvent) {
        let _ = self.inner.try_send(event);
    }
}

/// Receive side of the event queue.
pub struct EventReceiver {
    inner: Receiver<RuntimeEvent>,
}

impl EventReceiver {
    /// Try to receive an event without blocking.
    pub fn try_recv(&self) -> Option<RuntimeEvent> {
        self.inner.try_recv().ok()
    }

    /// Drain all pending events.
    pub fn drain(&self) -> impl Iterator<Item = RuntimeEvent> + '_ {
        std::iter::from_fn(|| self.try_recv())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ondeks_core::Event;

    #[test]
    fn event_queue_send_receive() {
        let queue = EventQueue::new(10);
        let sender = queue.sender();
        let receiver = queue.receiver();

        sender.send(RuntimeEvent::Underrun);
        let event = receiver.try_recv().unwrap();
        match event {
            RuntimeEvent::Underrun => {}
            _ => panic!("Wrong event"),
        }
    }

    #[test]
    fn event_queue_drain() {
        let queue = EventQueue::new(10);
        let sender = queue.sender();
        let receiver = queue.receiver();

        sender.send(RuntimeEvent::Underrun);
        sender.send(RuntimeEvent::MeterUpdate { left: 0.5, right: 0.5 });
        sender.send(RuntimeEvent::Core(Event::TransportStateChanged { is_playing: true }));

        let events: Vec<_> = receiver.drain().collect();
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn event_queue_drops_when_full() {
        let queue = EventQueue::new(1);
        let sender = queue.sender();
        let receiver = queue.receiver();

        sender.send(RuntimeEvent::Underrun);
        sender.send(RuntimeEvent::Underrun); // This should be dropped
        sender.send(RuntimeEvent::Underrun); // This should be dropped

        let events: Vec<_> = receiver.drain().collect();
        // Only one event should be received
        assert_eq!(events.len(), 1);
    }
}
