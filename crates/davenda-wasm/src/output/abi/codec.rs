use std::collections::{BTreeMap, BTreeSet};

use super::super::cache::{CacheVisibility, TypedCacheHint};
use super::super::json_ld::{JsonLdNode, JsonLdValue};
use super::super::metadata::TypedMetadata;
use super::cursor::{
    ByteCursor, read_option_string, write_len_u32, write_option_string, write_string, write_u8,
    write_u16, write_u32, write_u64,
};
use super::mapping::{
    extension_point_kind_from_tag, extension_point_kind_tag, robot_from_tag, robot_tag,
};
use super::{ABI_MAGIC, ABI_VERSION, TypedExecutionOutput, TypedResponseBody};
use crate::error::WasmModelError;

pub(super) fn encode_output(output: &TypedExecutionOutput) -> Result<Vec<u8>, WasmModelError> {
    let mut bytes = Vec::new();
    bytes.extend(ABI_MAGIC);
    write_u16(&mut bytes, ABI_VERSION);
    write_u8(&mut bytes, extension_point_kind_tag(output.surface));
    write_u16(&mut bytes, output.status);
    write_body(&mut bytes, &output.body)?;
    write_metadata(&mut bytes, &output.metadata)?;
    write_cache_hint(&mut bytes, output.cache_hint.as_ref())?;
    Ok(bytes)
}

pub(super) fn decode_output(bytes: &[u8]) -> Result<TypedExecutionOutput, WasmModelError> {
    let mut cursor = ByteCursor::new(bytes);
    let magic = cursor.read_array::<4>()?;
    if magic != ABI_MAGIC {
        return Err(WasmModelError::InvalidTypedReturn {
            reason: "typed return payload has an invalid magic header".to_string(),
        });
    }
    let version = cursor.read_u16()?;
    if version != ABI_VERSION {
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

    TypedExecutionOutput::new(surface, status, body, metadata, cache_hint)
}

fn write_body(bytes: &mut Vec<u8>, body: &TypedResponseBody) -> Result<(), WasmModelError> {
    match body {
        TypedResponseBody::HtmlDocument(html) => {
            write_u8(bytes, 0);
            write_string(bytes, html)?;
        }
        TypedResponseBody::HtmlFragment(html) => {
            write_u8(bytes, 1);
            write_string(bytes, html)?;
        }
        TypedResponseBody::JsonObject(payload) => {
            write_u8(bytes, 2);
            write_u32(
                bytes,
                write_len_u32("json_object_entry_count", payload.len())?,
            );
            for (key, value) in payload {
                write_string(bytes, key)?;
                write_string(bytes, value)?;
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
    write_option_string(bytes, metadata.title.as_ref())?;
    write_option_string(bytes, metadata.description.as_ref())?;
    write_option_string(bytes, metadata.canonical_url.as_ref())?;
    write_u32(
        bytes,
        write_len_u32("alternate_url_count", metadata.alternate_urls.len())?,
    );
    for (locale, url) in &metadata.alternate_urls {
        write_string(bytes, locale)?;
        write_string(bytes, url)?;
    }
    write_u32(bytes, write_len_u32("robots_count", metadata.robots.len())?);
    for directive in &metadata.robots {
        write_u8(bytes, robot_tag(*directive));
    }
    write_u32(
        bytes,
        write_len_u32("json_ld_node_count", metadata.json_ld.len())?,
    );
    for node in &metadata.json_ld {
        write_string(bytes, node.schema_type())?;
        write_u32(
            bytes,
            write_len_u32("json_ld_property_count", node.properties().len())?,
        );
        for (property, value) in node.properties() {
            write_string(bytes, property)?;
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
            node = node.insert_value(property, value)?;
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
            write_u32(bytes, write_len_u32("cache_tag_count", cache.tags.len())?);
            for tag in &cache.tags {
                write_string(bytes, tag)?;
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
            write_string(bytes, value)?;
        }
        JsonLdValue::Number(value) => {
            write_u8(bytes, 1);
            write_string(bytes, value)?;
        }
        JsonLdValue::Bool(value) => {
            write_u8(bytes, 2);
            write_u8(bytes, u8::from(*value));
        }
        JsonLdValue::Node(node) => {
            write_u8(bytes, 3);
            write_string(bytes, node.schema_type())?;
            write_u32(
                bytes,
                write_len_u32("json_ld_property_count", node.properties().len())?,
            );
            for (property, property_value) in node.properties() {
                write_string(bytes, property)?;
                write_json_ld_value(bytes, property_value)?;
            }
        }
        JsonLdValue::List(values) => {
            write_u8(bytes, 4);
            write_u32(
                bytes,
                write_len_u32("json_ld_list_item_count", values.len())?,
            );
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
                node = node.insert_value(property, value)?;
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
