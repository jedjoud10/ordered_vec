/// The message type
pub(crate) enum AtomicIndexedMessageType<T> {
    // Add the element at the specific index, if it's cell was of type "empty"
    Add(T, usize), 
    // Remove an element from the specific index, if it's cell was of tpye "valid"
    Remove(usize),
}