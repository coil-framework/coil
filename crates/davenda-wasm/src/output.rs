use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::error::WasmModelError;
use crate::ids::ExtensionPointKind;
use crate::validation::{require_non_empty, validate_token};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RobotsDirective {
    Index,
    NoIndex,
    Follow,
    NoFollow,
    NoArchive,
}

impl fmt::Display for RobotsDirective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Index => f.write_str("index"),
            Self::NoIndex => f.write_str("noindex"),
            Self::Follow => f.write_str("follow"),
            Self::NoFollow => f.write_str("nofollow"),
            Self::NoArchive => f.write_str("noarchive"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonLdValue {
    String(String),
    Number(String),
    Bool(bool),
    Node(JsonLdNode),
    List(Vec<JsonLdValue>),
}

impl JsonLdValue {
    fn render(&self) -> String {
        match self {
            Self::String(value) => format!("\"{}\"", escape_json(value)),
            Self::Number(value) => value.clone(),
            Self::Bool(value) => value.to_string(),
            Self::Node(node) => node.render(),
            Self::List(values) => format!(
                "[{}]",
                values
                    .iter()
                    .map(JsonLdValue::render)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonLdNode {
    schema_type: String,
    properties: BTreeMap<String, JsonLdValue>,
}

impl JsonLdNode {
    pub fn new(schema_type: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            schema_type: validate_token("schema_type", schema_type.into())?,
            properties: BTreeMap::new(),
        })
    }

    pub fn set_string(
        mut self,
        property: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(WasmModelError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(
            property,
            JsonLdValue::String(require_non_empty("json_ld_string", value.into())?),
        );
        Ok(self)
    }

    pub fn set_number(
        mut self,
        property: impl Into<String>,
        value: f64,
    ) -> Result<Self, WasmModelError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(WasmModelError::DuplicateJsonLdProperty { property });
        }
        if !value.is_finite() {
            return Err(WasmModelError::InvalidJsonLdNumber {
                property,
                value: value.to_string(),
            });
        }
        self.properties
            .insert(property, JsonLdValue::Number(value.to_string()));
        Ok(self)
    }

    pub fn set_bool(
        mut self,
        property: impl Into<String>,
        value: bool,
    ) -> Result<Self, WasmModelError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(WasmModelError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::Bool(value));
        Ok(self)
    }

    pub fn set_node(
        mut self,
        property: impl Into<String>,
        node: JsonLdNode,
    ) -> Result<Self, WasmModelError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(WasmModelError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::Node(node));
        Ok(self)
    }

    pub fn set_list(
        mut self,
        property: impl Into<String>,
        values: Vec<JsonLdValue>,
    ) -> Result<Self, WasmModelError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(WasmModelError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::List(values));
        Ok(self)
    }

    pub fn render(&self) -> String {
        let mut segments = vec![format!("\"@type\":\"{}\"", escape_json(&self.schema_type))];
        for (property, value) in &self.properties {
            segments.push(format!("\"{}\":{}", escape_json(property), value.render()));
        }
        format!("{{{}}}", segments.join(","))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypedMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical_url: Option<String>,
    pub alternate_urls: BTreeMap<String, String>,
    pub robots: BTreeSet<RobotsDirective>,
    pub json_ld: Vec<JsonLdNode>,
}

impl TypedMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Result<Self, WasmModelError> {
        self.title = Some(require_non_empty("title", title.into())?);
        Ok(self)
    }

    pub fn with_description(
        mut self,
        description: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        self.description = Some(require_non_empty("description", description.into())?);
        Ok(self)
    }

    pub fn with_canonical_url(
        mut self,
        canonical_url: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        self.canonical_url = Some(validate_absolute_url(
            "canonical_url",
            canonical_url.into(),
        )?);
        Ok(self)
    }

    pub fn insert_alternate_url(
        mut self,
        locale: impl Into<String>,
        url: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        let locale = validate_token("alternate_url_locale", locale.into())?;
        let url = validate_absolute_url("alternate_url", url.into())?;
        self.alternate_urls.insert(locale, url);
        Ok(self)
    }

    pub fn with_robot_directive(mut self, directive: RobotsDirective) -> Self {
        self.robots.insert(directive);
        self
    }

    pub fn push_json_ld(mut self, node: JsonLdNode) -> Self {
        self.json_ld.push(node);
        self
    }

    pub fn merge_from(&mut self, other: &Self) {
        if other.title.is_some() {
            self.title = other.title.clone();
        }
        if other.description.is_some() {
            self.description = other.description.clone();
        }
        if other.canonical_url.is_some() {
            self.canonical_url = other.canonical_url.clone();
        }
        for (locale, url) in &other.alternate_urls {
            self.alternate_urls.insert(locale.clone(), url.clone());
        }
        self.robots.extend(other.robots.iter().copied());
        self.json_ld.extend(other.json_ld.iter().cloned());
    }

    fn validate(&self) -> Result<(), WasmModelError> {
        if self
            .title
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
            || self
                .description
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "metadata title and description must be non-empty when present".to_string(),
            });
        }
        if let Some(canonical_url) = &self.canonical_url {
            let _ = validate_absolute_url("canonical_url", canonical_url.clone())?;
        }
        if self
            .alternate_urls
            .iter()
            .any(|(locale, url)| locale.trim().is_empty() || url.trim().is_empty())
        {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "metadata alternate URLs must be non-empty".to_string(),
            });
        }
        for (locale, url) in &self.alternate_urls {
            let _ = validate_token("alternate_url_locale", locale.clone())?;
            let _ = validate_absolute_url("alternate_url", url.clone())?;
        }
        for node in &self.json_ld {
            node.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedCacheHint {
    pub visibility: CacheVisibility,
    pub max_age_seconds: u64,
    pub stale_while_revalidate_seconds: Option<u64>,
    pub vary_by_locale: bool,
    pub vary_by_user: bool,
    pub vary_by_session: bool,
    pub tags: BTreeSet<String>,
}

impl TypedCacheHint {
    pub fn new(
        visibility: CacheVisibility,
        max_age_seconds: u64,
        stale_while_revalidate_seconds: Option<u64>,
        vary_by_locale: bool,
        vary_by_user: bool,
        vary_by_session: bool,
        tags: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, WasmModelError> {
        let tags = tags
            .into_iter()
            .map(|tag| validate_token("cache_tag", tag.into()))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let hint = Self {
            visibility,
            max_age_seconds,
            stale_while_revalidate_seconds,
            vary_by_locale,
            vary_by_user,
            vary_by_session,
            tags,
        };
        hint.validate()?;
        Ok(hint)
    }

    fn validate(&self) -> Result<(), WasmModelError> {
        if self.visibility == CacheVisibility::Public && (self.vary_by_user || self.vary_by_session)
        {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "public cache hints cannot vary by user or session".to_string(),
            });
        }
        if self
            .stale_while_revalidate_seconds
            .is_some_and(|value| value == 0)
        {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "stale-while-revalidate must be greater than zero".to_string(),
            });
        }
        for tag in &self.tags {
            let _ = validate_token("cache_tag", tag.clone())?;
        }
        Ok(())
    }

    pub fn merge_from(&mut self, other: &Self) {
        self.visibility = match (self.visibility, other.visibility) {
            (CacheVisibility::Private, _) | (_, CacheVisibility::Private) => {
                CacheVisibility::Private
            }
            _ => CacheVisibility::Public,
        };
        self.max_age_seconds = self.max_age_seconds.min(other.max_age_seconds);
        self.stale_while_revalidate_seconds = match (
            self.stale_while_revalidate_seconds,
            other.stale_while_revalidate_seconds,
        ) {
            (Some(left), Some(right)) => Some(left.min(right)),
            _ => None,
        };
        self.vary_by_locale |= other.vary_by_locale;
        self.vary_by_user |= other.vary_by_user;
        self.vary_by_session |= other.vary_by_session;
        self.tags.extend(other.tags.iter().cloned());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedResponseBody {
    HtmlDocument(String),
    HtmlFragment(String),
    JsonObject(BTreeMap<String, String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedExecutionOutput {
    pub surface: ExtensionPointKind,
    pub status: u16,
    pub body: TypedResponseBody,
    pub metadata: TypedMetadata,
    pub cache_hint: Option<TypedCacheHint>,
}

impl TypedExecutionOutput {
    pub const ABI_EXPORT: &'static str = "__davenda_typed_output";
    const ABI_MAGIC: [u8; 4] = *b"DVRO";
    const ABI_VERSION: u16 = 1;

    pub fn page(
        status: u16,
        body: impl Into<String>,
        metadata: TypedMetadata,
        cache_hint: Option<TypedCacheHint>,
    ) -> Result<Self, WasmModelError> {
        Self::new(
            ExtensionPointKind::Page,
            status,
            TypedResponseBody::HtmlDocument(require_non_empty("page_body", body.into())?),
            metadata,
            cache_hint,
        )
    }

    pub fn api(
        status: u16,
        payload: BTreeMap<String, String>,
        metadata: TypedMetadata,
        cache_hint: Option<TypedCacheHint>,
    ) -> Result<Self, WasmModelError> {
        Self::new(
            ExtensionPointKind::Api,
            status,
            TypedResponseBody::JsonObject(payload),
            metadata,
            cache_hint,
        )
    }

    pub fn admin_widget(
        status: u16,
        fragment: impl Into<String>,
        metadata: TypedMetadata,
        cache_hint: Option<TypedCacheHint>,
    ) -> Result<Self, WasmModelError> {
        Self::new(
            ExtensionPointKind::AdminWidget,
            status,
            TypedResponseBody::HtmlFragment(require_non_empty(
                "admin_widget_fragment",
                fragment.into(),
            )?),
            metadata,
            cache_hint,
        )
    }

    pub fn render_hook(
        status: u16,
        fragment: impl Into<String>,
        metadata: TypedMetadata,
        cache_hint: Option<TypedCacheHint>,
    ) -> Result<Self, WasmModelError> {
        Self::new(
            ExtensionPointKind::RenderHook,
            status,
            TypedResponseBody::HtmlFragment(require_non_empty(
                "render_hook_fragment",
                fragment.into(),
            )?),
            metadata,
            cache_hint,
        )
    }

    pub fn new(
        surface: ExtensionPointKind,
        status: u16,
        body: TypedResponseBody,
        metadata: TypedMetadata,
        cache_hint: Option<TypedCacheHint>,
    ) -> Result<Self, WasmModelError> {
        let output = Self {
            surface,
            status,
            body,
            metadata,
            cache_hint,
        };
        output.validate_for_point(surface)?;
        Ok(output)
    }

    pub fn decode_for_point(
        bytes: &[u8],
        point: ExtensionPointKind,
    ) -> Result<Self, WasmModelError> {
        let output = Self::decode(bytes)?;
        output.validate_for_point(point)?;
        Ok(output)
    }

    pub fn validate_for_point(&self, point: ExtensionPointKind) -> Result<(), WasmModelError> {
        if self.surface != point {
            return Err(WasmModelError::TypedReturnPointMismatch {
                expected: point,
                actual: self.surface,
            });
        }

        if let Some(expected_body_kind) = expected_body_kind_for_point(point) {
            if self.body_kind() != expected_body_kind {
                return Err(WasmModelError::TypedReturnBodyMismatch {
                    point,
                    body: self.body_kind(),
                });
            }
        } else {
            return Err(WasmModelError::TypedReturnBodyMismatch {
                point,
                body: self.body_kind(),
            });
        }

        self.validate()
    }

    pub fn encode(&self) -> Result<Vec<u8>, WasmModelError> {
        self.validate_for_point(self.surface)?;
        let mut bytes = Vec::new();
        bytes.extend(Self::ABI_MAGIC);
        write_u16(&mut bytes, Self::ABI_VERSION);
        write_u8(&mut bytes, extension_point_kind_tag(self.surface));
        write_u16(&mut bytes, self.status);
        write_body(&mut bytes, &self.body)?;
        write_metadata(&mut bytes, &self.metadata)?;
        write_cache_hint(&mut bytes, self.cache_hint.as_ref())?;
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, WasmModelError> {
        let mut cursor = ByteCursor::new(bytes);
        let magic = cursor.read_array::<4>()?;
        if magic != Self::ABI_MAGIC {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "typed return payload has an invalid magic header".to_string(),
            });
        }
        let version = cursor.read_u16()?;
        if version != Self::ABI_VERSION {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: format!("typed return payload version `{version}` is not supported"),
            });
        }

        let surface = extension_point_kind_from_tag(cursor.read_u8()?)?;
        let status = cursor.read_u16()?;
        let body = read_body(&mut cursor)?;
        let metadata = read_metadata(&mut cursor)?;
        let cache_hint = read_cache_hint(&mut cursor)?;
        if !cursor.is_empty() {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "typed return payload has trailing bytes".to_string(),
            });
        }

        Self::new(surface, status, body, metadata, cache_hint)
    }

    fn body_kind(&self) -> TypedResponseBodyKind {
        match &self.body {
            TypedResponseBody::HtmlDocument(_) => TypedResponseBodyKind::HtmlDocument,
            TypedResponseBody::HtmlFragment(_) => TypedResponseBodyKind::HtmlFragment,
            TypedResponseBody::JsonObject(_) => TypedResponseBodyKind::JsonObject,
        }
    }

    fn validate(&self) -> Result<(), WasmModelError> {
        validate_http_status(self.status)?;
        match &self.body {
            TypedResponseBody::HtmlDocument(html) | TypedResponseBody::HtmlFragment(html) => {
                let _ = require_non_empty("typed_response_body", html.clone())?;
            }
            TypedResponseBody::JsonObject(payload) => {
                for key in payload.keys() {
                    let _ = require_non_empty("json_object_key", key.clone())?;
                }
            }
        }
        self.metadata.validate()?;
        if let Some(cache) = &self.cache_hint {
            cache.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedResponseBodyKind {
    HtmlDocument,
    HtmlFragment,
    JsonObject,
}

impl fmt::Display for TypedResponseBodyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HtmlDocument => f.write_str("html_document"),
            Self::HtmlFragment => f.write_str("html_fragment"),
            Self::JsonObject => f.write_str("json_object"),
        }
    }
}

fn write_body(bytes: &mut Vec<u8>, body: &TypedResponseBody) -> Result<(), WasmModelError> {
    match body {
        TypedResponseBody::HtmlDocument(html) => {
            write_u8(bytes, 0);
            write_string(bytes, html);
        }
        TypedResponseBody::HtmlFragment(html) => {
            write_u8(bytes, 1);
            write_string(bytes, html);
        }
        TypedResponseBody::JsonObject(payload) => {
            write_u8(bytes, 2);
            write_u32(bytes, payload.len() as u32);
            for (key, value) in payload {
                write_string(bytes, key);
                write_string(bytes, value);
            }
        }
    }
    Ok(())
}

fn read_body(cursor: &mut ByteCursor<'_>) -> Result<TypedResponseBody, WasmModelError> {
    Ok(match cursor.read_u8()? {
        0 => TypedResponseBody::HtmlDocument(cursor.read_string()?),
        1 => TypedResponseBody::HtmlFragment(cursor.read_string()?),
        2 => {
            let count = cursor.read_u32()? as usize;
            let mut payload = BTreeMap::new();
            for _ in 0..count {
                let key = cursor.read_string()?;
                let value = cursor.read_string()?;
                if payload.insert(key.clone(), value).is_some() {
                    return Err(WasmModelError::InvalidTypedReturn {
                        reason: format!("duplicate JSON response key `{key}`"),
                    });
                }
            }
            TypedResponseBody::JsonObject(payload)
        }
        tag => {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: format!("typed response body tag `{tag}` is not supported"),
            });
        }
    })
}

fn write_metadata(bytes: &mut Vec<u8>, metadata: &TypedMetadata) -> Result<(), WasmModelError> {
    write_option_string(bytes, metadata.title.as_ref());
    write_option_string(bytes, metadata.description.as_ref());
    write_option_string(bytes, metadata.canonical_url.as_ref());
    write_u32(bytes, metadata.alternate_urls.len() as u32);
    for (locale, url) in &metadata.alternate_urls {
        write_string(bytes, locale);
        write_string(bytes, url);
    }
    write_u32(bytes, metadata.robots.len() as u32);
    for directive in &metadata.robots {
        write_u8(bytes, robot_tag(*directive));
    }
    write_u32(bytes, metadata.json_ld.len() as u32);
    for node in &metadata.json_ld {
        write_string(bytes, &node.schema_type);
        write_u32(bytes, node.properties.len() as u32);
        for (property, value) in &node.properties {
            write_string(bytes, property);
            write_json_ld_value(bytes, value)?;
        }
    }
    Ok(())
}

fn read_metadata(cursor: &mut ByteCursor<'_>) -> Result<TypedMetadata, WasmModelError> {
    let title = read_option_string(cursor)?;
    let description = read_option_string(cursor)?;
    let canonical_url = read_option_string(cursor)?;
    let alternate_len = cursor.read_u32()? as usize;
    let mut alternate_urls = BTreeMap::new();
    for _ in 0..alternate_len {
        let locale = cursor.read_string()?;
        let url = cursor.read_string()?;
        if alternate_urls.insert(locale.clone(), url).is_some() {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: format!("duplicate alternate URL locale `{locale}`"),
            });
        }
    }
    let robots_len = cursor.read_u32()? as usize;
    let mut robots = BTreeSet::new();
    for _ in 0..robots_len {
        robots.insert(robot_from_tag(cursor.read_u8()?)?);
    }
    let json_ld_len = cursor.read_u32()? as usize;
    let mut json_ld = Vec::with_capacity(json_ld_len);
    for _ in 0..json_ld_len {
        let schema_type = cursor.read_string()?;
        let property_len = cursor.read_u32()? as usize;
        let mut node = JsonLdNode::new(schema_type)?;
        for _ in 0..property_len {
            let property = cursor.read_string()?;
            let value = read_json_ld_value(cursor)?;
            if node.properties.insert(property.clone(), value).is_some() {
                return Err(WasmModelError::InvalidTypedReturn {
                    reason: format!("duplicate JSON-LD property `{property}`"),
                });
            }
        }
        json_ld.push(node);
    }

    Ok(TypedMetadata {
        title,
        description,
        canonical_url,
        alternate_urls,
        robots,
        json_ld,
    })
}

fn write_cache_hint(
    bytes: &mut Vec<u8>,
    cache_hint: Option<&TypedCacheHint>,
) -> Result<(), WasmModelError> {
    match cache_hint {
        Some(cache) => {
            write_u8(bytes, 1);
            write_u8(
                bytes,
                match cache.visibility {
                    CacheVisibility::Public => 0,
                    CacheVisibility::Private => 1,
                },
            );
            write_u64(bytes, cache.max_age_seconds);
            match cache.stale_while_revalidate_seconds {
                Some(value) => {
                    write_u8(bytes, 1);
                    write_u64(bytes, value);
                }
                None => write_u8(bytes, 0),
            }
            let mut flags = 0u8;
            if cache.vary_by_locale {
                flags |= 0b001;
            }
            if cache.vary_by_user {
                flags |= 0b010;
            }
            if cache.vary_by_session {
                flags |= 0b100;
            }
            write_u8(bytes, flags);
            write_u32(bytes, cache.tags.len() as u32);
            for tag in &cache.tags {
                write_string(bytes, tag);
            }
        }
        None => write_u8(bytes, 0),
    }
    Ok(())
}

fn read_cache_hint(cursor: &mut ByteCursor<'_>) -> Result<Option<TypedCacheHint>, WasmModelError> {
    if cursor.read_u8()? == 0 {
        return Ok(None);
    }

    let visibility = match cursor.read_u8()? {
        0 => CacheVisibility::Public,
        1 => CacheVisibility::Private,
        other => {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: format!("typed cache visibility tag `{other}` is not supported"),
            });
        }
    };
    let max_age_seconds = cursor.read_u64()?;
    let stale_while_revalidate_seconds = if cursor.read_u8()? == 0 {
        None
    } else {
        Some(cursor.read_u64()?)
    };
    let flags = cursor.read_u8()?;
    let tag_len = cursor.read_u32()? as usize;
    let mut tags = BTreeSet::new();
    for _ in 0..tag_len {
        let tag = cursor.read_string()?;
        if !tags.insert(tag.clone()) {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: format!("duplicate cache tag `{tag}`"),
            });
        }
    }

    Ok(Some(TypedCacheHint {
        visibility,
        max_age_seconds,
        stale_while_revalidate_seconds,
        vary_by_locale: flags & 0b001 != 0,
        vary_by_user: flags & 0b010 != 0,
        vary_by_session: flags & 0b100 != 0,
        tags,
    }))
}

fn write_json_ld_value(bytes: &mut Vec<u8>, value: &JsonLdValue) -> Result<(), WasmModelError> {
    match value {
        JsonLdValue::String(value) => {
            write_u8(bytes, 0);
            write_string(bytes, value);
        }
        JsonLdValue::Number(value) => {
            write_u8(bytes, 1);
            write_string(bytes, value);
        }
        JsonLdValue::Bool(value) => {
            write_u8(bytes, 2);
            write_u8(bytes, u8::from(*value));
        }
        JsonLdValue::Node(node) => {
            write_u8(bytes, 3);
            write_string(bytes, &node.schema_type);
            write_u32(bytes, node.properties.len() as u32);
            for (property, property_value) in &node.properties {
                write_string(bytes, property);
                write_json_ld_value(bytes, property_value)?;
            }
        }
        JsonLdValue::List(values) => {
            write_u8(bytes, 4);
            write_u32(bytes, values.len() as u32);
            for item in values {
                write_json_ld_value(bytes, item)?;
            }
        }
    }
    Ok(())
}

fn read_json_ld_value(cursor: &mut ByteCursor<'_>) -> Result<JsonLdValue, WasmModelError> {
    Ok(match cursor.read_u8()? {
        0 => JsonLdValue::String(cursor.read_string()?),
        1 => JsonLdValue::Number(cursor.read_string()?),
        2 => JsonLdValue::Bool(match cursor.read_u8()? {
            0 => false,
            1 => true,
            other => {
                return Err(WasmModelError::InvalidTypedReturn {
                    reason: format!("typed JSON-LD boolean tag `{other}` is invalid"),
                });
            }
        }),
        3 => {
            let schema_type = cursor.read_string()?;
            let property_len = cursor.read_u32()? as usize;
            let mut node = JsonLdNode::new(schema_type)?;
            for _ in 0..property_len {
                let property = cursor.read_string()?;
                let value = read_json_ld_value(cursor)?;
                if node.properties.insert(property.clone(), value).is_some() {
                    return Err(WasmModelError::InvalidTypedReturn {
                        reason: format!("duplicate JSON-LD property `{property}`"),
                    });
                }
            }
            JsonLdValue::Node(node)
        }
        4 => {
            let count = cursor.read_u32()? as usize;
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(read_json_ld_value(cursor)?);
            }
            JsonLdValue::List(values)
        }
        tag => {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: format!("typed JSON-LD value tag `{tag}` is invalid"),
            });
        }
    })
}

fn robot_tag(directive: RobotsDirective) -> u8 {
    match directive {
        RobotsDirective::Index => 0,
        RobotsDirective::NoIndex => 1,
        RobotsDirective::Follow => 2,
        RobotsDirective::NoFollow => 3,
        RobotsDirective::NoArchive => 4,
    }
}

fn robot_from_tag(tag: u8) -> Result<RobotsDirective, WasmModelError> {
    match tag {
        0 => Ok(RobotsDirective::Index),
        1 => Ok(RobotsDirective::NoIndex),
        2 => Ok(RobotsDirective::Follow),
        3 => Ok(RobotsDirective::NoFollow),
        4 => Ok(RobotsDirective::NoArchive),
        other => Err(WasmModelError::InvalidTypedReturn {
            reason: format!("typed robots directive tag `{other}` is invalid"),
        }),
    }
}

fn extension_point_kind_from_tag(tag: u8) -> Result<ExtensionPointKind, WasmModelError> {
    match tag {
        0 => Ok(ExtensionPointKind::Page),
        1 => Ok(ExtensionPointKind::Api),
        2 => Ok(ExtensionPointKind::Job),
        3 => Ok(ExtensionPointKind::ScheduledJob),
        4 => Ok(ExtensionPointKind::Webhook),
        5 => Ok(ExtensionPointKind::AdminWidget),
        6 => Ok(ExtensionPointKind::RenderHook),
        other => Err(WasmModelError::InvalidTypedReturn {
            reason: format!("typed surface tag `{other}` is invalid"),
        }),
    }
}

fn extension_point_kind_tag(kind: ExtensionPointKind) -> u8 {
    match kind {
        ExtensionPointKind::Page => 0,
        ExtensionPointKind::Api => 1,
        ExtensionPointKind::Job => 2,
        ExtensionPointKind::ScheduledJob => 3,
        ExtensionPointKind::Webhook => 4,
        ExtensionPointKind::AdminWidget => 5,
        ExtensionPointKind::RenderHook => 6,
    }
}

fn validate_property_name(value: String) -> Result<String, WasmModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '@' | '_' | '-'))
    {
        Err(WasmModelError::InvalidJsonLdProperty {
            property: trimmed.to_string(),
        })
    } else {
        Ok(trimmed.to_string())
    }
}

fn is_absolute_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

impl JsonLdNode {
    fn validate(&self) -> Result<(), WasmModelError> {
        let _ = validate_token("schema_type", self.schema_type.clone())?;
        for (property, value) in &self.properties {
            let _ = validate_property_name(property.clone())?;
            value.validate()?;
        }
        Ok(())
    }
}

impl JsonLdValue {
    fn validate(&self) -> Result<(), WasmModelError> {
        match self {
            Self::String(value) => {
                let _ = require_non_empty("json_ld_string", value.clone())?;
            }
            Self::Number(value) => {
                if value
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .is_none()
                {
                    return Err(WasmModelError::InvalidJsonLdNumber {
                        property: "json_ld_number".to_string(),
                        value: value.clone(),
                    });
                }
            }
            Self::Bool(_) => {}
            Self::Node(node) => node.validate()?,
            Self::List(values) => {
                for value in values {
                    value.validate()?;
                }
            }
        }
        Ok(())
    }
}

fn validate_absolute_url(field: &'static str, value: String) -> Result<String, WasmModelError> {
    let trimmed = require_non_empty(field, value)?;
    if is_absolute_http_url(&trimmed) {
        Ok(trimmed)
    } else {
        Err(WasmModelError::InvalidTypedReturn {
            reason: format!("`{field}` must be an absolute URL"),
        })
    }
}

fn expected_body_kind_for_point(point: ExtensionPointKind) -> Option<TypedResponseBodyKind> {
    match point {
        ExtensionPointKind::Page => Some(TypedResponseBodyKind::HtmlDocument),
        ExtensionPointKind::Api => Some(TypedResponseBodyKind::JsonObject),
        ExtensionPointKind::AdminWidget | ExtensionPointKind::RenderHook => {
            Some(TypedResponseBodyKind::HtmlFragment)
        }
        ExtensionPointKind::Job
        | ExtensionPointKind::ScheduledJob
        | ExtensionPointKind::Webhook => None,
    }
}

fn validate_http_status(status: u16) -> Result<(), WasmModelError> {
    if (100..=599).contains(&status) {
        Ok(())
    } else {
        Err(WasmModelError::InvalidTypedStatus { status })
    }
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_u8(bytes: &mut Vec<u8>, value: u8) {
    bytes.push(value);
}

fn write_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend(value.to_le_bytes());
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    write_u32(bytes, value.len() as u32);
    bytes.extend(value.as_bytes());
}

fn write_option_string(bytes: &mut Vec<u8>, value: Option<&String>) {
    match value {
        Some(value) => {
            write_u8(bytes, 1);
            write_string(bytes, value);
        }
        None => write_u8(bytes, 0),
    }
}

fn read_option_string(cursor: &mut ByteCursor<'_>) -> Result<Option<String>, WasmModelError> {
    Ok(match cursor.read_u8()? {
        0 => None,
        1 => Some(cursor.read_string()?),
        other => {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: format!("typed optional string tag `{other}` is invalid"),
            });
        }
    })
}

#[derive(Debug)]
struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn is_empty(&self) -> bool {
        self.offset >= self.bytes.len()
    }

    fn read_u8(&mut self) -> Result<u8, WasmModelError> {
        if self.offset >= self.bytes.len() {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "unexpected end of typed return payload".to_string(),
            });
        }
        let value = self.bytes[self.offset];
        self.offset += 1;
        Ok(value)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], WasmModelError> {
        if self.offset + N > self.bytes.len() {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "unexpected end of typed return payload".to_string(),
            });
        }
        let mut out = [0u8; N];
        out.copy_from_slice(&self.bytes[self.offset..self.offset + N]);
        self.offset += N;
        Ok(out)
    }

    fn read_u16(&mut self) -> Result<u16, WasmModelError> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }

    fn read_u32(&mut self) -> Result<u32, WasmModelError> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    fn read_u64(&mut self) -> Result<u64, WasmModelError> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    fn read_string(&mut self) -> Result<String, WasmModelError> {
        let len = self.read_u32()? as usize;
        if self.offset + len > self.bytes.len() {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "typed return payload string extends past buffer".to_string(),
            });
        }
        let bytes = &self.bytes[self.offset..self.offset + len];
        self.offset += len;
        let value =
            std::str::from_utf8(bytes).map_err(|error| WasmModelError::InvalidTypedReturn {
                reason: error.to_string(),
            })?;
        Ok(value.to_string())
    }
}
