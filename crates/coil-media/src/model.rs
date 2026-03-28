use crate::error::MediaModelError;
use crate::validation::require_non_empty;
use coil_assets::ContentFingerprint;
use coil_storage::{DeliveryMode, Sensitivity, StoragePolicy};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationStatus {
    Draft,
    Published,
    Unpublished,
    Archived,
}

impl fmt::Display for PublicationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => f.write_str("draft"),
            Self::Published => f.write_str("published"),
            Self::Unpublished => f.write_str("unpublished"),
            Self::Archived => f.write_str("archived"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaMetadata {
    pub title: String,
    pub alt_text: Option<String>,
    pub caption: Option<String>,
    pub description: Option<String>,
    pub credit: Option<String>,
    pub copyright: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub tags: BTreeSet<String>,
}

impl MediaMetadata {
    pub fn new(title: impl Into<String>) -> Result<Self, MediaModelError> {
        Ok(Self {
            title: require_non_empty("media_title", title.into())?,
            alt_text: None,
            caption: None,
            description: None,
            credit: None,
            copyright: None,
            width: None,
            height: None,
            tags: BTreeSet::new(),
        })
    }

    pub fn with_alt_text(mut self, alt_text: impl Into<String>) -> Result<Self, MediaModelError> {
        self.alt_text = Some(require_non_empty("media_alt_text", alt_text.into())?);
        Ok(self)
    }

    pub fn with_caption(mut self, caption: impl Into<String>) -> Result<Self, MediaModelError> {
        self.caption = Some(require_non_empty("media_caption", caption.into())?);
        Ok(self)
    }

    pub fn with_description(
        mut self,
        description: impl Into<String>,
    ) -> Result<Self, MediaModelError> {
        self.description = Some(require_non_empty("media_description", description.into())?);
        Ok(self)
    }

    pub fn with_credit(mut self, credit: impl Into<String>) -> Result<Self, MediaModelError> {
        self.credit = Some(require_non_empty("media_credit", credit.into())?);
        Ok(self)
    }

    pub fn with_copyright(mut self, copyright: impl Into<String>) -> Result<Self, MediaModelError> {
        self.copyright = Some(require_non_empty("media_copyright", copyright.into())?);
        Ok(self)
    }

    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Result<Self, MediaModelError> {
        self.tags
            .insert(crate::validation::validate_token("media_tag", tag.into())?);
        Ok(self)
    }

    pub fn image_dimensions(&self) -> Option<(u32, u32)> {
        self.width.zip(self.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTechnicalMetadata {
    pub content_type: String,
    pub byte_length: u64,
    pub fingerprint: ContentFingerprint,
}

impl MediaTechnicalMetadata {
    pub fn new(
        content_type: impl Into<String>,
        byte_length: u64,
        fingerprint: ContentFingerprint,
    ) -> Result<Self, MediaModelError> {
        Ok(Self {
            content_type: require_non_empty("content_type", content_type.into())?,
            byte_length,
            fingerprint,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaDerivativeKind {
    Thumbnail,
    Preview,
    Responsive,
    Archive,
    Custom,
}

impl fmt::Display for MediaDerivativeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Thumbnail => f.write_str("thumbnail"),
            Self::Preview => f.write_str("preview"),
            Self::Responsive => f.write_str("responsive"),
            Self::Archive => f.write_str("archive"),
            Self::Custom => f.write_str("custom"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaDerivative {
    pub id: crate::MediaDerivativeId,
    pub kind: MediaDerivativeKind,
    pub title: String,
    pub mime_type: String,
    pub storage_policy: StoragePolicy,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl MediaDerivative {
    pub fn new(
        id: crate::MediaDerivativeId,
        kind: MediaDerivativeKind,
        title: impl Into<String>,
        mime_type: impl Into<String>,
        storage_policy: StoragePolicy,
    ) -> Result<Self, MediaModelError> {
        storage_policy.validate().map_err(MediaModelError::from)?;

        Ok(Self {
            id,
            kind,
            title: require_non_empty("derivative_title", title.into())?,
            mime_type: require_non_empty("derivative_mime_type", mime_type.into())?,
            storage_policy,
            width: None,
            height: None,
        })
    }

    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }
}

pub(crate) fn is_public_revision(storage_policy: &StoragePolicy) -> bool {
    storage_policy.delivery_mode == DeliveryMode::PublicCdn
        && storage_policy.sensitivity == Sensitivity::Public
}
