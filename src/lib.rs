//! `shapoist-core`: The core functions of Shapoist.
//!
//! Warning: Shapoist is still under developing, with each new version having breaking changes.
//! 

pub const DELAY_ADJUSTMENT: &[u8; 653816] = include_bytes!("../delay_adjustment.mp3");
pub const CLICK_SOUND: &[u8; 8446] = include_bytes!("../click_sound.mp3");

pub mod system;