use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockFieldValueKind {
    PlainText,
    RichText,
    Boolean,
    AssetReference,
    Path,
}

impl fmt::Display for BlockFieldValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlainText => f.write_str("plain_text"),
            Self::RichText => f.write_str("rich_text"),
            Self::Boolean => f.write_str("boolean"),
            Self::AssetReference => f.write_str("asset_reference"),
            Self::Path => f.write_str("path"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockFieldValue {
    PlainText(String),
    RichText(String),
    Boolean(bool),
    AssetReference(AssetReference),
    Path(String),
}

impl BlockFieldValue {
    pub fn plain_text(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self::PlainText(require_non_empty(
            "block_field_value",
            value.into(),
        )?))
    }

    pub fn rich_text(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self::RichText(require_non_empty(
            "block_field_value",
            value.into(),
        )?))
    }

    pub fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    pub fn asset_reference(value: AssetReference) -> Self {
        Self::AssetReference(value)
    }

    pub fn path(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self::Path(validate_path(
            "block_field_value",
            value.into(),
        )?))
    }

    pub fn kind(&self) -> BlockFieldValueKind {
        match self {
            Self::PlainText(_) => BlockFieldValueKind::PlainText,
            Self::RichText(_) => BlockFieldValueKind::RichText,
            Self::Boolean(_) => BlockFieldValueKind::Boolean,
            Self::AssetReference(_) => BlockFieldValueKind::AssetReference,
            Self::Path(_) => BlockFieldValueKind::Path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockFieldSchema {
    pub id: BlockFieldId,
    pub label: String,
    pub kind: BlockFieldValueKind,
    pub required: bool,
    pub multiple: bool,
}

impl BlockFieldSchema {
    pub fn new(
        id: BlockFieldId,
        label: impl Into<String>,
        kind: BlockFieldValueKind,
        required: bool,
        multiple: bool,
    ) -> Result<Self, CmsModelError> {
        Ok(Self {
            id,
            label: require_non_empty("block_field_label", label.into())?,
            kind,
            required,
            multiple,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredBlockInstance {
    pub id: BlockInstanceId,
    pub block_type: BlockTypeId,
    pub fields: BTreeMap<BlockFieldId, Vec<BlockFieldValue>>,
}

impl StructuredBlockInstance {
    pub fn new(
        id: BlockInstanceId,
        block_type: BlockTypeId,
        fields: BTreeMap<BlockFieldId, Vec<BlockFieldValue>>,
    ) -> Result<Self, CmsModelError> {
        for (field_id, values) in &fields {
            if values.is_empty() {
                return Err(CmsModelError::EmptyBlockFieldValues {
                    block_type_id: block_type.to_string(),
                    field_id: field_id.to_string(),
                });
            }
        }

        Ok(Self {
            id,
            block_type,
            fields,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTypeSchema {
    pub id: BlockTypeId,
    pub label: String,
    fields: Vec<BlockFieldSchema>,
}

impl BlockTypeSchema {
    pub fn new(
        id: BlockTypeId,
        label: impl Into<String>,
        fields: Vec<BlockFieldSchema>,
    ) -> Result<Self, CmsModelError> {
        let label = require_non_empty("block_type_label", label.into())?;
        let mut seen = BTreeSet::new();
        for field in &fields {
            if !seen.insert(field.id.clone()) {
                return Err(CmsModelError::DuplicateBlockFieldSchema {
                    block_type_id: id.to_string(),
                    field_id: field.id.to_string(),
                });
            }
        }

        Ok(Self { id, label, fields })
    }

    pub fn fields(&self) -> &[BlockFieldSchema] {
        &self.fields
    }

    pub fn instantiate(
        &self,
        instance_id: BlockInstanceId,
        fields: BTreeMap<BlockFieldId, Vec<BlockFieldValue>>,
    ) -> Result<StructuredBlockInstance, CmsModelError> {
        let instance = StructuredBlockInstance::new(instance_id, self.id.clone(), fields)?;
        self.validate_instance(&instance)?;
        Ok(instance)
    }

    pub fn validate_instance(
        &self,
        instance: &StructuredBlockInstance,
    ) -> Result<(), CmsModelError> {
        if instance.block_type != self.id {
            return Err(CmsModelError::BlockTypeMismatch {
                expected_block_type_id: self.id.to_string(),
                actual_block_type_id: instance.block_type.to_string(),
            });
        }

        let field_schemas = self
            .fields
            .iter()
            .map(|field| (field.id.clone(), field))
            .collect::<BTreeMap<_, _>>();

        for (field_id, values) in &instance.fields {
            let schema =
                field_schemas
                    .get(field_id)
                    .ok_or_else(|| CmsModelError::UnknownBlockField {
                        block_type_id: self.id.to_string(),
                        field_id: field_id.to_string(),
                    })?;

            if values.is_empty() {
                return Err(CmsModelError::EmptyBlockFieldValues {
                    block_type_id: self.id.to_string(),
                    field_id: field_id.to_string(),
                });
            }

            if !schema.multiple && values.len() > 1 {
                return Err(CmsModelError::BlockFieldDoesNotAllowMultiple {
                    block_type_id: self.id.to_string(),
                    field_id: field_id.to_string(),
                });
            }

            for value in values {
                let actual = value.kind();
                if actual != schema.kind {
                    return Err(CmsModelError::InvalidBlockFieldValueKind {
                        block_type_id: self.id.to_string(),
                        field_id: field_id.to_string(),
                        expected: schema.kind,
                        actual,
                    });
                }
            }
        }

        for field in &self.fields {
            if field.required && !instance.fields.contains_key(&field.id) {
                return Err(CmsModelError::MissingRequiredBlockField {
                    block_type_id: self.id.to_string(),
                    field_id: field.id.to_string(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedBlock {
    pub id: SharedBlockId,
    pub label: String,
    pub block: StructuredBlockInstance,
}

impl SharedBlock {
    pub fn new(
        id: SharedBlockId,
        label: impl Into<String>,
        block: StructuredBlockInstance,
    ) -> Result<Self, CmsModelError> {
        Ok(Self {
            id,
            label: require_non_empty("shared_block_label", label.into())?,
            block,
        })
    }

    pub fn reference(&self, instance_id: BlockInstanceId) -> SharedBlockReference {
        SharedBlockReference {
            instance_id,
            shared_block_id: self.id.clone(),
            block_type: self.block.block_type.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedBlockReference {
    pub instance_id: BlockInstanceId,
    pub shared_block_id: SharedBlockId,
    pub block_type: BlockTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageBlockInstance {
    Inline(StructuredBlockInstance),
    Shared(SharedBlockReference),
}

impl PageBlockInstance {
    pub fn instance_id(&self) -> &BlockInstanceId {
        match self {
            Self::Inline(block) => &block.id,
            Self::Shared(reference) => &reference.instance_id,
        }
    }

    pub fn block_type(&self) -> &BlockTypeId {
        match self {
            Self::Inline(block) => &block.block_type,
            Self::Shared(reference) => &reference.block_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageOptions {
    pub show_in_navigation: bool,
    pub allow_indexing: bool,
    pub include_in_sitemap: bool,
}

impl Default for PageOptions {
    fn default() -> Self {
        Self {
            show_in_navigation: true,
            allow_indexing: true,
            include_in_sitemap: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageSettings {
    pub navigation_label: Option<String>,
    pub layout_variant: Option<String>,
    pub options: PageOptions,
}

impl PageSettings {
    pub fn new(options: PageOptions) -> Self {
        Self {
            navigation_label: None,
            layout_variant: None,
            options,
        }
    }

    pub fn with_navigation_label(
        mut self,
        navigation_label: impl Into<String>,
    ) -> Result<Self, CmsModelError> {
        self.navigation_label = Some(require_non_empty(
            "page_navigation_label",
            navigation_label.into(),
        )?);
        Ok(self)
    }

    pub fn with_layout_variant(
        mut self,
        layout_variant: impl Into<String>,
    ) -> Result<Self, CmsModelError> {
        self.layout_variant = Some(validate_token(
            "page_layout_variant",
            layout_variant.into(),
        )?);
        Ok(self)
    }
}
