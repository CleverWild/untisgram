use teloxide::prelude::ChatId;

use crate::{message::Diagnostic, work::Chat};

pub mod cleanup;
pub mod diff_impl;
pub mod message;
pub mod message_formatter;
pub mod status;
pub mod utils;
pub mod work;

#[cfg(test)]
mod test_support;

pub const IS_PROD: bool = !cfg!(debug_assertions);

/// My telegram DM
pub const DEBUG_TELEGRAM_CHAT: Chat<Diagnostic> = Chat::diagnostic(ChatId(690963502), None);
