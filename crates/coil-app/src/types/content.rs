use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentFieldType {
    Text,
    RichText,
    Slug,
    Boolean,
    Integer,
    DateTime,
    Asset,
    Reference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentField {
    pub id: ContentFieldId,
    pub field_type: ContentFieldType,
    pub localized: bool,
    pub required: bool,
}

impl ContentField {
    pub fn new(id: impl Into<String>, field_type: ContentFieldType) -> Result<Self, AppModelError> {
        Ok(Self {
            id: ContentFieldId::new(id.into())?,
            field_type,
            localized: false,
            required: false,
        })
    }

    pub fn localized(mut self) -> Self {
        self.localized = true;
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentModel {
    pub id: ContentModelId,
    pub resource_kind: String,
    pub fields: Vec<ContentField>,
}

impl ContentModel {
    pub fn new(
        id: impl Into<String>,
        resource_kind: impl Into<String>,
        fields: Vec<ContentField>,
    ) -> Result<Self, AppModelError> {
        if fields.is_empty() {
            return Err(AppModelError::EmptyField {
                field: "content_model_fields",
            });
        }

        let id = ContentModelId::new(id.into())?;
        let resource_kind = require_non_empty("content_model_resource_kind", resource_kind.into())?;
        let mut seen = BTreeSet::new();
        for field in &fields {
            if !seen.insert(field.id.to_string()) {
                return Err(AppModelError::DuplicateContentField {
                    model_id: id.to_string(),
                    field_id: field.id.to_string(),
                });
            }
        }

        Ok(Self {
            id,
            resource_kind,
            fields,
        })
    }
}
