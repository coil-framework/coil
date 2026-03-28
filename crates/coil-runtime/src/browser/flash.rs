use super::support::validate_browser_value;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl FlashLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    fn parse(value: &str) -> Result<Self, RuntimeBrowserError> {
        match value {
            "info" => Ok(Self::Info),
            "success" => Ok(Self::Success),
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            other => Err(RuntimeBrowserError::InvalidFlashLevel {
                level: other.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashMessage {
    pub level: FlashLevel,
    pub text: String,
}

impl FlashMessage {
    pub fn new(level: FlashLevel, text: impl Into<String>) -> Result<Self, RuntimeBrowserError> {
        let text = validate_browser_value("flash_message", text.into())?;
        Ok(Self { level, text })
    }
}

pub(super) fn serialize_flash_messages(
    messages: &[FlashMessage],
) -> Result<String, RuntimeBrowserError> {
    let mut payload = String::new();
    for message in messages {
        if message.text.trim().is_empty() {
            return Err(RuntimeBrowserError::EmptyValue {
                field: "flash_message",
            });
        }

        payload.push_str(message.level.as_str());
        payload.push(':');
        payload.push_str(&message.text.len().to_string());
        payload.push(':');
        payload.push_str(&message.text);
        payload.push('|');
    }

    Ok(payload)
}

pub(super) fn deserialize_flash_messages(
    payload: &str,
) -> Result<Vec<FlashMessage>, RuntimeBrowserError> {
    let mut remaining = payload;
    let mut messages = Vec::new();

    while !remaining.is_empty() {
        let Some(level_sep) = remaining.find(':') else {
            return Err(RuntimeBrowserError::InvalidFlashPayload);
        };
        let level = &remaining[..level_sep];
        remaining = &remaining[level_sep + 1..];

        let Some(length_sep) = remaining.find(':') else {
            return Err(RuntimeBrowserError::InvalidFlashPayload);
        };
        let length: usize = remaining[..length_sep]
            .parse()
            .map_err(|_| RuntimeBrowserError::InvalidFlashPayload)?;
        remaining = &remaining[length_sep + 1..];

        if remaining.len() < length + 1 {
            return Err(RuntimeBrowserError::InvalidFlashPayload);
        }

        let message = &remaining[..length];
        let separator = &remaining[length..length + 1];
        if separator != "|" {
            return Err(RuntimeBrowserError::InvalidFlashPayload);
        }

        messages.push(FlashMessage::new(FlashLevel::parse(level)?, message)?);
        remaining = &remaining[length + 1..];
    }

    Ok(messages)
}
