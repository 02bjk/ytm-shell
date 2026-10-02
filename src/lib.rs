#![deny(unsafe_code)]

pub mod adblock;
pub mod allowlist;
#[cfg(target_os = "linux")]
pub mod mpris;
pub mod network_block;
pub mod platform;
pub mod player;
pub mod shortcuts;
pub mod tray;
