use std::sync::atomic::AtomicU64;

use super::message::AtomicIndexedMessageType;

/// Counter that keeps track of the amount of commands that we have sent
static COMMAND_COUNTER: AtomicU64 = AtomicU64::new(0);
/// Some channel command that we can send to the creation thread
pub(crate) struct AtomicIndexedCommand<T> {
    // Command ID, and Message Type
    pub(crate) command_id: usize,
    pub(crate) message: AtomicIndexedMessageType<T>,
}

impl<T> AtomicIndexedCommand<T> {
    pub(crate) fn new(command_id: usize, message: AtomicIndexedMessageType<T>) -> Self {
        Self { command_id, message }
    }
}
