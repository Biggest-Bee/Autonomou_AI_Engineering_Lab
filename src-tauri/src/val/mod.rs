// Virtualization Abstraction Layer (VAL)
// Platform-specific virtualization drivers for Linux KVM, macOS vfkit, and Windows WSL2/Hyper-V

pub mod driver;
pub mod network;
pub mod terminal;
pub mod process_tracker;

pub use driver::{ValDriver, MicroVMConfig, NetworkInterface};
pub use network::{NetworkStack, ChaosConfig};
pub use terminal::TerminalManager;
pub use process_tracker::{ProcessTracker, ResourceGovernor, ResourceLimits, TrackedProcess};