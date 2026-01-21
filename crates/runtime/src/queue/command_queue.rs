//! Command queue for sending commands to the audio thread.

use crossbeam_channel::{bounded, Sender, Receiver, TrySendError};
use ondeks_core::Command;
use crate::audio::AudioConfig;

/// Commands that can be sent to the audio engine.
#[derive(Debug, Clone)]
pub enum RuntimeCommand {
    /// Core engine command.
    Core(Command),
    /// Request to shut down.
    Shutdown,
    /// Audio configuration change.
    SetAudioConfig(AudioConfig),
}

/// Error sending a command.
#[derive(Debug)]
pub enum QueueError {
    /// Queue is full.
    Full,
    /// Queue is disconnected.
    Disconnected,
}

/// Queue for sending commands to the audio thread.
pub struct CommandQueue {
    sender: Sender<RuntimeCommand>,
    receiver: Receiver<RuntimeCommand>,
}

impl CommandQueue {
    /// Create a new command queue with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        Self { sender, receiver }
    }

    /// Get the sender (for non-audio threads).
    pub fn sender(&self) -> CommandSender {
        CommandSender {
            inner: self.sender.clone(),
        }
    }

    /// Get the receiver (for audio thread).
    pub fn receiver(&self) -> CommandReceiver {
        CommandReceiver {
            inner: self.receiver.clone(),
        }
    }
}

/// Send side of the command queue (cloneable).
#[derive(Clone)]
pub struct CommandSender {
    inner: Sender<RuntimeCommand>,
}

impl CommandSender {
    /// Send a command. Non-blocking.
    pub fn send(&self, command: RuntimeCommand) -> Result<(), QueueError> {
        self.inner.try_send(command).map_err(|e| match e {
            TrySendError::Full(_) => QueueError::Full,
            TrySendError::Disconnected(_) => QueueError::Disconnected,
        })
    }
}

/// Receive side of the command queue.
pub struct CommandReceiver {
    inner: Receiver<RuntimeCommand>,
}

impl CommandReceiver {
    /// Try to receive a command without blocking.
    pub fn try_recv(&self) -> Option<RuntimeCommand> {
        self.inner.try_recv().ok()
    }

    /// Drain all pending commands.
    pub fn drain(&self) -> impl Iterator<Item = RuntimeCommand> + '_ {
        std::iter::from_fn(|| self.try_recv())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ondeks_core::Command;

    #[test]
    fn command_queue_send_receive() {
        let queue = CommandQueue::new(10);
        let sender = queue.sender();
        let receiver = queue.receiver();

        sender.send(RuntimeCommand::Core(Command::Play)).unwrap();
        let cmd = receiver.try_recv().unwrap();
        match cmd {
            RuntimeCommand::Core(Command::Play) => {}
            _ => panic!("Wrong command"),
        }
    }

    #[test]
    fn command_queue_drain() {
        let queue = CommandQueue::new(10);
        let sender = queue.sender();
        let receiver = queue.receiver();

        sender.send(RuntimeCommand::Core(Command::Play)).unwrap();
        sender.send(RuntimeCommand::Core(Command::Stop)).unwrap();
        sender.send(RuntimeCommand::Shutdown).unwrap();

        let commands: Vec<_> = receiver.drain().collect();
        assert_eq!(commands.len(), 3);
    }

    #[test]
    fn command_queue_full_error() {
        let queue = CommandQueue::new(1);
        let sender = queue.sender();

        sender.send(RuntimeCommand::Shutdown).unwrap();
        assert!(matches!(
            sender.send(RuntimeCommand::Shutdown),
            Err(QueueError::Full)
        ));
    }
}
