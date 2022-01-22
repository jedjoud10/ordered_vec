// Export
#![feature(map_first_last)]
mod ordered_vec;
mod unversioned_ordered_vec;
mod shareable_ordered_vec;
mod test;
mod utils;
pub mod simple {
    pub use super::ordered_vec::*;
    pub use super::unversioned_ordered_vec::*;
}
pub mod shareable {
    pub use super::shareable_ordered_vec::*;
}
