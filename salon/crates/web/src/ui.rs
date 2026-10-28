//! Small owned browser resources and pure guards for UI lifetimes.
//!
//! A listener or timer lives with the object that stores it. An async result
//! belongs to one operation, and a one-shot effect to an increasing identity.
//! Neither the task guard nor the effect gate decides the game's semantics.

mod frames;
mod guards;
mod resources;

pub use frames::AnimationLoop;
pub use guards::{Composition, OnceEffects, Operation, OperationScope, OperationSlot};
pub use resources::{Listener, Listeners, Timer};
