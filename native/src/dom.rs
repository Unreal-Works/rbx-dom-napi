#![allow(dead_code)]

use std::collections::{BTreeMap, HashSet};
use std::io::Cursor;
use std::str::FromStr;
use std::sync::{Arc, Mutex, MutexGuard};

use napi::bindgen_prelude::Buffer;
use napi::Result;
use napi_derive::napi;
use rbx_binary::{CompressionType, Deserializer, Serializer};
use rbx_dom_weak::{
    ustr, DomViewer as UpstreamDomViewer, InstanceBuilder as UpstreamInstanceBuilder, WeakDom,
};
use rbx_reflection::ReflectionDatabase as UpstreamReflectionDatabase;
use rbx_reflection_database::get_bundled;
use rbx_types::{Ref, Variant};
use rbx_xml::{DecodeOptions, DecodePropertyBehavior, EncodeOptions, EncodePropertyBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use xml::reader::{ParserConfig, XmlEvent};

use crate::error::{catch_panic, invalid_arg, upstream_error};
use crate::reflection::{normalize_database_json, ReflectionDatabase};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub(crate) struct IoOptions {
    #[serde(rename = "propertyBehavior", alias = "property_behavior")]
    pub(crate) property_behavior: Option<String>,
    pub(crate) compression: Option<String>,
    #[serde(rename = "includeRoot", alias = "include_root")]
    pub(crate) include_root: bool,
    pub(crate) refs: Option<Vec<String>>,
    #[serde(rename = "reflectionDatabase", alias = "reflection_database")]
    pub(crate) reflection_database: Option<Value>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct InstanceSpec {
    #[serde(rename = "className", alias = "class")]
    class_name: Option<String>,
    name: Option<String>,
    referent: Option<String>,
    properties: BTreeMap<String, Value>,
    children: Vec<InstanceSpec>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InstanceView {
    referent: String,
    parent: String,
    children: Vec<String>,
    name: String,
    class_name: String,
    properties: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DomSnapshot {
    root_ref: String,
    instances: Vec<InstanceView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDom {
    root_ref: String,
    instances: BTreeMap<String, InstanceView>,
}

pub(crate) struct DecodedXml {
    pub(crate) dom: WeakDom,
    pub(crate) source_referents: BTreeMap<String, String>,
}

struct SourceItem {
    path: Vec<usize>,
    referent: String,
}

struct SourceItemPosition {
    path: Vec<usize>,
    child_count: usize,
}

struct SourceElement {
    is_root: bool,
    is_item: bool,
    allows_items: bool,
}

fn parse_io_options(options_json: Option<&str>) -> Result<IoOptions> {
    match options_json {
        Some(options) => serde_json::from_str(options)
            .map_err(|error| invalid_arg(format!("invalid I/O options JSON: {error}"))),
        None => Ok(IoOptions::default()),
    }
}

fn parse_ref(value: &str) -> Result<Ref> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return Ok(Ref::none());
    }
    Ref::from_str(value)
        .map_err(|error| invalid_arg(format!("invalid referent {value:?}: {error}")))
}

pub(crate) fn ref_string(value: Ref) -> String {
    value.to_string()
}

fn normalize_variant_value(value: Value) -> Value {
    let Value::Object(mut object) = value else {
        return value;
    };

    if let Some(Value::String(raw)) = object.get("Int64") {
        if let Ok(value) = raw.parse::<i64>() {
            object.insert("Int64".to_owned(), Value::Number(value.into()));
        }
    }
    if let Some(Value::String(raw)) = object.get("SecurityCapabilities") {
        if let Ok(value) = raw.parse::<u64>() {
            object.insert(
                "SecurityCapabilities".to_owned(),
                Value::Number(value.into()),
            );
        }
    }

    Value::Object(object)
}

pub(crate) fn parse_variant(value: Value) -> Result<Variant> {
    let normalized = normalize_variant_value(value.clone());
    if let Ok(variant) = serde_json::from_value::<Variant>(normalized) {
        return Ok(variant);
    }

    if let Value::Object(mut object) = value.clone() {
        if let (Some(Value::String(variant_type)), Some(variant_value)) =
            (object.remove("type"), object.remove("value"))
        {
            let mut tagged = serde_json::Map::new();
            tagged.insert(variant_type, normalize_variant_value(variant_value));
            if let Ok(variant) = serde_json::from_value::<Variant>(Value::Object(tagged)) {
                return Ok(variant);
            }
        }
    }

    match value {
        Value::Bool(value) => Ok(Variant::Bool(value)),
        Value::String(value) => Ok(Variant::String(value)),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                if let Ok(value) = i32::try_from(value) {
                    Ok(Variant::Int32(value))
                } else {
                    Ok(Variant::Int64(value))
                }
            } else if let Some(value) = value.as_f64() {
                Ok(Variant::Float64(value))
            } else {
                Err(invalid_arg("numeric property is not finite"))
            }
        }
        Value::Null => Err(invalid_arg(
            "null is not an rbx-dom Variant; use an explicitly tagged OptionalCFrame instead",
        )),
        other => Err(invalid_arg(format!(
            "property value must be an upstream Variant object, primitive, or typed object; got {other}"
        ))),
    }
}

pub(crate) fn variant_to_value(value: &Variant) -> Result<Value> {
    match value {
        Variant::Int64(value) => Ok(serde_json::json!({ "Int64": value.to_string() })),
        Variant::SecurityCapabilities(value) => {
            Ok(serde_json::json!({ "SecurityCapabilities": value.bits().to_string() }))
        }
        _ => serde_json::to_value(value)
            .map_err(|error| upstream_error("serializing Variant", error)),
    }
}

fn ensure_unique_refs(refs: &[Ref]) -> Result<()> {
    let mut seen = HashSet::with_capacity(refs.len());
    for referent in refs {
        if referent.is_none() || !seen.insert(*referent) {
            return Err(invalid_arg(format!(
                "instance specifications must contain unique non-empty referents; found {referent}"
            )));
        }
    }
    Ok(())
}

fn build_instance(spec: InstanceSpec) -> Result<(UpstreamInstanceBuilder, Vec<Ref>)> {
    let class_name = spec.class_name.unwrap_or_else(|| "Folder".to_owned());
    let mut builder = UpstreamInstanceBuilder::new(ustr(&class_name));

    if let Some(referent) = spec.referent {
        let referent = parse_ref(&referent)?;
        if referent.is_some() {
            builder = builder.with_referent(referent);
        }
    }
    if let Some(name) = spec.name {
        builder = builder.with_name(name);
    }
    for (name, value) in spec.properties {
        builder.add_property(name, parse_variant(value)?);
    }
    let mut referents = vec![builder.referent()];
    for child in spec.children {
        let (child, child_referents) = build_instance(child)?;
        builder.add_child(child);
        referents.extend(child_referents);
    }

    ensure_unique_refs(&referents)?;
    Ok((builder, referents))
}

pub(crate) fn dom_from_spec_json(spec_json: &str) -> Result<WeakDom> {
    let spec: InstanceSpec = serde_json::from_str(spec_json)
        .map_err(|error| invalid_arg(format!("invalid instance specification JSON: {error}")))?;
    let (builder, referents) = build_instance(spec)?;
    ensure_unique_refs(&referents)?;
    Ok(WeakDom::new(builder))
}

fn instance_view(instance: &rbx_dom_weak::Instance) -> Result<InstanceView> {
    let properties = instance
        .properties
        .iter()
        .map(|(name, value)| Ok((name.to_string(), variant_to_value(value)?)))
        .collect::<Result<_>>()?;

    Ok(InstanceView {
        referent: ref_string(instance.referent()),
        parent: ref_string(instance.parent()),
        children: instance
            .children()
            .iter()
            .copied()
            .map(ref_string)
            .collect(),
        name: instance.name.clone(),
        class_name: instance.class.to_string(),
        properties,
    })
}

fn snapshot_value(dom: &WeakDom) -> Result<DomSnapshot> {
    let instances = if dom.root_ref().is_none() {
        Vec::new()
    } else {
        dom.descendants()
            .map(instance_view)
            .collect::<Result<Vec<_>>>()?
    };
    Ok(DomSnapshot {
        root_ref: ref_string(dom.root_ref()),
        instances,
    })
}

fn snapshot_json(dom: &WeakDom) -> Result<String> {
    serde_json::to_string(&snapshot_value(dom)?)
        .map_err(|error| upstream_error("serializing DOM snapshot", error))
}

fn instance_json(dom: &WeakDom, referent: Ref) -> Result<Option<String>> {
    let Some(instance) = dom.get_by_ref(referent) else {
        return Ok(None);
    };
    serde_json::to_string(&instance_view(instance)?)
        .map(Some)
        .map_err(|error| upstream_error("serializing instance", error))
}

fn parse_property_behavior(value: Option<&str>) -> Result<Option<DecodePropertyBehavior>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let behavior = match value {
        "ignoreUnknown" | "IgnoreUnknown" => DecodePropertyBehavior::IgnoreUnknown,
        "readUnknown" | "ReadUnknown" => DecodePropertyBehavior::ReadUnknown,
        "errorOnUnknown" | "ErrorOnUnknown" => DecodePropertyBehavior::ErrorOnUnknown,
        "noReflection" | "NoReflection" => DecodePropertyBehavior::NoReflection,
        _ => {
            return Err(invalid_arg(format!(
                "unknown XML property behavior {value:?}"
            )))
        }
    };
    Ok(Some(behavior))
}

fn parse_encode_property_behavior(value: Option<&str>) -> Result<Option<EncodePropertyBehavior>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let behavior = match value {
        "ignoreUnknown" | "IgnoreUnknown" => EncodePropertyBehavior::IgnoreUnknown,
        "writeUnknown" | "WriteUnknown" => EncodePropertyBehavior::WriteUnknown,
        "errorOnUnknown" | "ErrorOnUnknown" => EncodePropertyBehavior::ErrorOnUnknown,
        "noReflection" | "NoReflection" => EncodePropertyBehavior::NoReflection,
        _ => {
            return Err(invalid_arg(format!(
                "unknown XML property behavior {value:?}"
            )))
        }
    };
    Ok(Some(behavior))
}

fn parse_compression(value: Option<&str>) -> Result<CompressionType> {
    match value.unwrap_or("lz4") {
        "lz4" | "Lz4" => Ok(CompressionType::Lz4),
        "none" | "None" => Ok(CompressionType::None),
        "zstd" | "Zstd" => Ok(CompressionType::Zstd),
        value => Err(invalid_arg(format!("unknown binary compression {value:?}"))),
    }
}

fn refs_for(dom: &WeakDom, options: &IoOptions) -> Result<Vec<Ref>> {
    if dom.root_ref().is_none() {
        return Err(invalid_arg("cannot serialize an empty DOM"));
    }
    if let Some(refs) = &options.refs {
        return refs.iter().map(|value| parse_ref(value)).collect();
    }
    if options.include_root {
        Ok(vec![dom.root_ref()])
    } else {
        Ok(dom.root().children().to_vec())
    }
}

fn validate_insert(dom: &WeakDom, parent: Ref, referents: &[Ref]) -> Result<()> {
    if parent.is_some() && dom.get_by_ref(parent).is_none() {
        return Err(invalid_arg(format!(
            "cannot insert into parent {parent}; it is not in this DOM"
        )));
    }
    for referent in referents {
        if dom.get_by_ref(*referent).is_some() {
            return Err(invalid_arg(format!(
                "cannot insert duplicate referent {referent}"
            )));
        }
    }
    Ok(())
}

fn subtree_contains(dom: &WeakDom, root: Ref, target: Ref) -> bool {
    let mut pending = vec![root];
    let mut visited = HashSet::new();
    while let Some(current) = pending.pop() {
        if current == target {
            return true;
        }
        if !visited.insert(current) {
            continue;
        }
        if let Some(instance) = dom.get_by_ref(current) {
            pending.extend(instance.children().iter().copied());
        }
    }
    false
}

fn validate_transfer(dom: &WeakDom, referent: Ref, parent: Ref) -> Result<()> {
    if referent == dom.root_ref() {
        return Err(invalid_arg("cannot transfer the DOM root"));
    }
    if dom.get_by_ref(referent).is_none() {
        return Err(invalid_arg(format!(
            "cannot transfer missing instance {referent}"
        )));
    }
    if parent.is_some() && dom.get_by_ref(parent).is_none() {
        return Err(invalid_arg(format!(
            "cannot transfer into missing parent {parent}"
        )));
    }
    if parent.is_some() && subtree_contains(dom, referent, parent) {
        return Err(invalid_arg(
            "cannot transfer an instance into itself or one of its descendants",
        ));
    }
    Ok(())
}

fn collect_source_referents(data: &[u8], dom: &WeakDom) -> Result<BTreeMap<String, String>> {
    let reader = ParserConfig::new()
        .ignore_comments(true)
        .create_reader(Cursor::new(data));
    let mut items = Vec::new();
    let mut stack: Vec<SourceItemPosition> = Vec::new();
    let mut elements: Vec<SourceElement> = Vec::new();
    let mut root_count = 0;

    for event in reader {
        match event.map_err(|error| upstream_error("reading XML source referents", error))? {
            XmlEvent::StartElement {
                name, attributes, ..
            } => {
                let is_item = name.local_name == "Item"
                    && elements.last().is_some_and(|element| element.allows_items);

                if is_item {
                    let path = if let Some(parent) = stack.last_mut() {
                        let mut path = parent.path.clone();
                        path.push(parent.child_count);
                        parent.child_count += 1;
                        path
                    } else {
                        let path = vec![root_count];
                        root_count += 1;
                        path
                    };

                    if let Some(referent) = attributes
                        .into_iter()
                        .filter(|attribute| attribute.name.local_name == "referent")
                        .map(|attribute| attribute.value)
                        .next_back()
                    {
                        items.push(SourceItem {
                            path: path.clone(),
                            referent,
                        });
                    }
                    stack.push(SourceItemPosition {
                        path,
                        child_count: 0,
                    });
                }

                let is_root = elements.is_empty() && name.local_name == "roblox";
                elements.push(SourceElement {
                    is_root,
                    is_item,
                    allows_items: is_root || is_item,
                });
            }
            XmlEvent::EndElement { .. } => {
                if let Some(element) = elements.pop() {
                    if element.is_item {
                        stack.pop();
                    }
                    if element.is_root {
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    items
        .into_iter()
        .map(|item| {
            let mut instance = dom.root();
            for child_index in item.path {
                let child = instance.children().get(child_index).ok_or_else(|| {
                    crate::error::failure("decoded DOM does not match XML item structure")
                })?;
                instance = dom.get_by_ref(*child).ok_or_else(|| {
                    crate::error::failure("decoded DOM contains a missing child referent")
                })?;
            }
            Ok((ref_string(instance.referent()), item.referent))
        })
        .collect()
}

pub(crate) fn decode_xml_bytes(data: Vec<u8>, options: &IoOptions) -> Result<DecodedXml> {
    catch_panic("rbx_xml::from_reader", || {
        if let Some(database_json) = options.reflection_database.as_ref() {
            let database_json = normalize_database_json(database_json)?;
            let database: UpstreamReflectionDatabase<'_> = serde_json::from_str(&database_json)
                .map_err(|error| {
                    invalid_arg(format!("invalid reflection database JSON: {error}"))
                })?;
            decode_xml_with_database(data, options, &database)
        } else {
            decode_xml_with_database(data, options, get_bundled())
        }
    })
}

fn decode_xml_with_database(
    data: Vec<u8>,
    options: &IoOptions,
    database: &UpstreamReflectionDatabase<'_>,
) -> Result<DecodedXml> {
    let mut decoder = DecodeOptions::new().reflection_database(database);
    if let Some(behavior) = parse_property_behavior(options.property_behavior.as_deref())? {
        decoder = decoder.property_behavior(behavior);
    }
    let dom = rbx_xml::from_reader(Cursor::new(data.as_slice()), decoder)
        .map_err(|error| upstream_error("rbx_xml::from_reader", error))?;
    let source_referents = collect_source_referents(&data, &dom)?;
    Ok(DecodedXml {
        dom,
        source_referents,
    })
}

pub(crate) fn encode_xml_bytes(dom: &WeakDom, options: &IoOptions) -> Result<Vec<u8>> {
    catch_panic("rbx_xml::to_writer", || {
        if let Some(database_json) = options.reflection_database.as_ref() {
            let database_json = normalize_database_json(database_json)?;
            let database: UpstreamReflectionDatabase<'_> = serde_json::from_str(&database_json)
                .map_err(|error| {
                    invalid_arg(format!("invalid reflection database JSON: {error}"))
                })?;
            encode_xml_with_database(dom, options, &database)
        } else {
            encode_xml_with_database(dom, options, get_bundled())
        }
    })
}

fn encode_xml_with_database(
    dom: &WeakDom,
    options: &IoOptions,
    database: &UpstreamReflectionDatabase<'_>,
) -> Result<Vec<u8>> {
    let mut encoder = EncodeOptions::new().reflection_database(database);
    if let Some(behavior) = parse_encode_property_behavior(options.property_behavior.as_deref())? {
        encoder = encoder.property_behavior(behavior);
    }
    let refs = refs_for(dom, options)?;
    let mut output = Vec::new();
    rbx_xml::to_writer(&mut output, dom, &refs, encoder)
        .map_err(|error| upstream_error("rbx_xml::to_writer", error))?;
    Ok(output)
}

pub(crate) fn decode_binary_bytes(data: Vec<u8>, options: &IoOptions) -> Result<WeakDom> {
    catch_panic("rbx_binary::from_reader", || {
        if let Some(database_json) = options.reflection_database.as_ref() {
            let database_json = normalize_database_json(database_json)?;
            let database: UpstreamReflectionDatabase<'_> = serde_json::from_str(&database_json)
                .map_err(|error| {
                    invalid_arg(format!("invalid reflection database JSON: {error}"))
                })?;
            decode_binary_with_database(data, &database)
        } else {
            decode_binary_with_database(data, get_bundled())
        }
    })
}

fn decode_binary_with_database(
    data: Vec<u8>,
    database: &UpstreamReflectionDatabase<'_>,
) -> Result<WeakDom> {
    Deserializer::new()
        .reflection_database(database)
        .deserialize(Cursor::new(data))
        .map_err(|error| upstream_error("rbx_binary::from_reader", error))
}

pub(crate) fn encode_binary_bytes(dom: &WeakDom, options: &IoOptions) -> Result<Vec<u8>> {
    catch_panic("rbx_binary::to_writer", || {
        let compression = parse_compression(options.compression.as_deref())?;
        if let Some(database_json) = options.reflection_database.as_ref() {
            let database_json = normalize_database_json(database_json)?;
            let database: UpstreamReflectionDatabase<'_> = serde_json::from_str(&database_json)
                .map_err(|error| {
                    invalid_arg(format!("invalid reflection database JSON: {error}"))
                })?;
            encode_binary_with_database(dom, options, compression, &database)
        } else {
            encode_binary_with_database(dom, options, compression, get_bundled())
        }
    })
}

fn encode_binary_with_database(
    dom: &WeakDom,
    options: &IoOptions,
    compression: CompressionType,
    database: &UpstreamReflectionDatabase<'_>,
) -> Result<Vec<u8>> {
    let encoder = Serializer::new()
        .reflection_database(database)
        .compression_type(compression);
    let refs = refs_for(dom, options)?;
    let mut output = Vec::new();
    encoder
        .serialize(&mut output, dom, &refs)
        .map_err(|error| upstream_error("rbx_binary::to_writer", error))?;
    Ok(output)
}

fn raw_instance_builder(
    referent: &str,
    raw: &BTreeMap<String, InstanceView>,
    visiting: &mut HashSet<String>,
) -> Result<(UpstreamInstanceBuilder, Vec<Ref>)> {
    if !visiting.insert(referent.to_owned()) {
        return Err(invalid_arg(format!(
            "raw DOM contains a child cycle at {referent}"
        )));
    }
    let view = raw
        .get(referent)
        .ok_or_else(|| invalid_arg(format!("raw DOM is missing instance {referent}")))?;
    let parsed_ref = parse_ref(referent)?;
    if parsed_ref.is_none() {
        return Err(invalid_arg("raw DOM instance referents cannot be empty"));
    }
    let mut builder = UpstreamInstanceBuilder::new(ustr(&view.class_name))
        .with_referent(parsed_ref)
        .with_name(view.name.clone());
    for (name, value) in &view.properties {
        builder.add_property(name.clone(), parse_variant(value.clone())?);
    }

    let mut referents = vec![parsed_ref];
    for child_ref in &view.children {
        let (child, child_referents) = raw_instance_builder(child_ref, raw, visiting)?;
        builder.add_child(child);
        referents.extend(child_referents);
    }
    visiting.remove(referent);
    Ok((builder, referents))
}

fn dom_from_raw_json(raw_json: &str) -> Result<WeakDom> {
    let raw: RawDom = serde_json::from_str(raw_json)
        .map_err(|error| invalid_arg(format!("invalid raw DOM JSON: {error}")))?;
    let (builder, referents) =
        raw_instance_builder(&raw.root_ref, &raw.instances, &mut HashSet::new())?;
    ensure_unique_refs(&referents)?;
    if referents.len() != raw.instances.len() {
        return Err(invalid_arg(
            "raw DOM contains instances that are not descendants of rootRef",
        ));
    }
    Ok(WeakDom::new(builder))
}

fn raw_instances_value(dom: &WeakDom) -> Result<BTreeMap<String, InstanceView>> {
    dom.descendants()
        .map(|instance| instance_view(instance).map(|view| (view.referent.clone(), view)))
        .collect()
}

fn raw_json(dom: &WeakDom) -> Result<String> {
    let raw = serde_json::json!({
        "rootRef": ref_string(dom.root_ref()),
        "instances": raw_instances_value(dom)?,
    });
    serde_json::to_string(&raw).map_err(|error| upstream_error("serializing raw DOM", error))
}

fn lock_dom(inner: &Arc<Mutex<WeakDom>>) -> Result<MutexGuard<'_, WeakDom>> {
    inner
        .lock()
        .map_err(|_| crate::error::failure("DOM lock is poisoned"))
}

fn subtree_refs(dom: &WeakDom, root: Ref) -> Vec<Ref> {
    dom.descendants_of(root)
        .map(|instance| instance.referent())
        .collect()
}

#[napi]
pub struct InstanceBuilder {
    inner: Option<UpstreamInstanceBuilder>,
    referents: Vec<Ref>,
    class_name: String,
    name: String,
    properties: BTreeMap<String, Value>,
}

#[napi]
impl InstanceBuilder {
    #[napi(constructor)]
    pub fn new(class_name: Option<String>, property_capacity: Option<u32>) -> Self {
        let class_name = class_name.unwrap_or_default();
        let inner = if let Some(capacity) = property_capacity {
            UpstreamInstanceBuilder::with_property_capacity(ustr(&class_name), capacity as usize)
        } else if class_name.is_empty() {
            UpstreamInstanceBuilder::empty()
        } else {
            UpstreamInstanceBuilder::new(ustr(&class_name))
        };
        let name = if class_name.is_empty() {
            String::new()
        } else {
            class_name.clone()
        };
        let referent = inner.referent();
        Self {
            inner: Some(inner),
            referents: vec![referent],
            class_name,
            name,
            properties: BTreeMap::new(),
        }
    }

    #[napi(js_name = "referent")]
    pub fn referent(&self) -> Result<String> {
        Ok(ref_string(
            self.inner
                .as_ref()
                .ok_or_else(|| invalid_arg("InstanceBuilder has already been consumed"))?
                .referent(),
        ))
    }

    #[napi(js_name = "className")]
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    #[napi(js_name = "name")]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[napi(js_name = "setClass")]
    pub fn set_class(&mut self, class_name: String) -> Result<()> {
        self.inner_mut()?.set_class(ustr(&class_name));
        self.class_name = class_name;
        Ok(())
    }

    #[napi(js_name = "setName")]
    pub fn set_name(&mut self, name: String) -> Result<()> {
        self.inner_mut()?.set_name(name.clone());
        self.name = name;
        Ok(())
    }

    #[napi(js_name = "setReferent")]
    pub fn set_referent(&mut self, referent: String) -> Result<()> {
        let referent = parse_ref(&referent)?;
        if referent.is_none() {
            return Err(invalid_arg("InstanceBuilder referents cannot be empty"));
        }
        self.inner = Some(
            self.inner
                .take()
                .ok_or_else(|| invalid_arg("InstanceBuilder has already been consumed"))?
                .with_referent(referent),
        );
        self.referents[0] = referent;
        Ok(())
    }

    #[napi(js_name = "hasProperty")]
    pub fn has_property(&self, property: String) -> Result<bool> {
        Ok(self
            .inner
            .as_ref()
            .ok_or_else(|| invalid_arg("InstanceBuilder has already been consumed"))?
            .has_property(property))
    }

    #[napi(js_name = "getProperty")]
    pub fn get_property(&self, property: String) -> Option<String> {
        self.properties
            .get(&property)
            .and_then(|value| serde_json::to_string(value).ok())
    }

    #[napi(js_name = "setProperty")]
    pub fn set_property(&mut self, property: String, value_json: String) -> Result<()> {
        let value: Value = serde_json::from_str(&value_json)
            .map_err(|error| invalid_arg(format!("invalid property JSON: {error}")))?;
        let variant = parse_variant(value)?;
        self.inner_mut()?
            .add_property(property.clone(), variant.clone());
        self.properties
            .insert(property, variant_to_value(&variant)?);
        Ok(())
    }

    #[napi(js_name = "addProperty")]
    pub fn add_property(&mut self, property: String, value_json: String) -> Result<()> {
        self.set_property(property, value_json)
    }

    #[napi(js_name = "addChild")]
    pub fn add_child(&mut self, child: &mut InstanceBuilder) -> Result<()> {
        let child_inner = child
            .inner
            .take()
            .ok_or_else(|| invalid_arg("child InstanceBuilder has already been consumed"))?;
        self.inner_mut()?.add_child(child_inner);
        self.referents.extend(child.referents.iter().copied());
        Ok(())
    }

    fn inner_mut(&mut self) -> Result<&mut UpstreamInstanceBuilder> {
        self.inner
            .as_mut()
            .ok_or_else(|| invalid_arg("InstanceBuilder has already been consumed"))
    }
}

#[napi]
pub struct Instance {
    dom: Arc<Mutex<WeakDom>>,
    referent: Ref,
}

#[napi]
impl Instance {
    #[napi(js_name = "referent")]
    pub fn referent(&self) -> String {
        ref_string(self.referent)
    }

    #[napi(js_name = "parent")]
    pub fn parent(&self) -> Result<String> {
        let dom = lock_dom(&self.dom)?;
        let instance = dom.get_by_ref(self.referent).ok_or_else(|| {
            invalid_arg(format!(
                "instance {} is no longer in this DOM",
                self.referent
            ))
        })?;
        Ok(ref_string(instance.parent()))
    }

    #[napi(js_name = "children")]
    pub fn children(&self) -> Result<Vec<String>> {
        let dom = lock_dom(&self.dom)?;
        let instance = dom.get_by_ref(self.referent).ok_or_else(|| {
            invalid_arg(format!(
                "instance {} is no longer in this DOM",
                self.referent
            ))
        })?;
        Ok(instance
            .children()
            .iter()
            .copied()
            .map(ref_string)
            .collect())
    }

    #[napi(js_name = "name")]
    pub fn name(&self) -> Result<String> {
        Ok(lock_dom(&self.dom)?
            .get_by_ref(self.referent)
            .ok_or_else(|| invalid_arg("instance is no longer in this DOM"))?
            .name
            .clone())
    }

    #[napi(js_name = "className")]
    pub fn class_name(&self) -> Result<String> {
        Ok(lock_dom(&self.dom)?
            .get_by_ref(self.referent)
            .ok_or_else(|| invalid_arg("instance is no longer in this DOM"))?
            .class
            .to_string())
    }

    #[napi(js_name = "snapshot")]
    pub fn snapshot(&self) -> Result<String> {
        let dom = lock_dom(&self.dom)?;
        instance_json(&dom, self.referent)?.ok_or_else(|| {
            invalid_arg(format!(
                "instance {} is no longer in this DOM",
                self.referent
            ))
        })
    }

    #[napi(js_name = "properties")]
    pub fn properties(&self) -> Result<String> {
        let dom = lock_dom(&self.dom)?;
        let instance = dom
            .get_by_ref(self.referent)
            .ok_or_else(|| invalid_arg("instance is no longer in this DOM"))?;
        let properties = instance
            .properties
            .iter()
            .map(|(name, value)| Ok((name.to_string(), variant_to_value(value)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        serde_json::to_string(&properties)
            .map_err(|error| upstream_error("serializing instance properties", error))
    }

    #[napi(js_name = "getProperty")]
    pub fn get_property(&self, property: String) -> Result<Option<String>> {
        let dom = lock_dom(&self.dom)?;
        let Some(instance) = dom.get_by_ref(self.referent) else {
            return Ok(None);
        };
        instance
            .properties
            .get(&ustr(&property))
            .map(|value| {
                serde_json::to_string(&variant_to_value(value)?)
                    .map_err(|error| upstream_error("serializing instance property", error))
            })
            .transpose()
    }

    #[napi(js_name = "setProperty")]
    pub fn set_property(&self, property: String, value_json: String) -> Result<()> {
        if property == "Name" || property == "ClassName" {
            return Err(invalid_arg("Name and ClassName are instance fields"));
        }
        let value: Value = serde_json::from_str(&value_json)
            .map_err(|error| invalid_arg(format!("invalid property JSON: {error}")))?;
        let mut dom = lock_dom(&self.dom)?;
        let instance = dom
            .get_by_ref_mut(self.referent)
            .ok_or_else(|| invalid_arg("instance is no longer in this DOM"))?;
        instance
            .properties
            .insert(ustr(&property), parse_variant(value)?);
        Ok(())
    }

    #[napi(js_name = "removeProperty")]
    pub fn remove_property(&self, property: String) -> Result<bool> {
        if property == "Name" || property == "ClassName" {
            return Err(invalid_arg("Name and ClassName are instance fields"));
        }
        let mut dom = lock_dom(&self.dom)?;
        Ok(dom
            .get_by_ref_mut(self.referent)
            .ok_or_else(|| invalid_arg("instance is no longer in this DOM"))?
            .properties
            .remove(&ustr(&property))
            .is_some())
    }

    #[napi(js_name = "setName")]
    pub fn set_name(&self, name: String) -> Result<()> {
        let mut dom = lock_dom(&self.dom)?;
        dom.get_by_ref_mut(self.referent)
            .ok_or_else(|| invalid_arg("instance is no longer in this DOM"))?
            .name = name;
        Ok(())
    }

    #[napi(js_name = "setClass")]
    pub fn set_class(&self, class_name: String) -> Result<()> {
        let mut dom = lock_dom(&self.dom)?;
        dom.get_by_ref_mut(self.referent)
            .ok_or_else(|| invalid_arg("instance is no longer in this DOM"))?
            .class = ustr(&class_name);
        Ok(())
    }
}

#[napi]
pub struct DomViewer {
    inner: UpstreamDomViewer,
}

#[napi]
impl DomViewer {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: UpstreamDomViewer::new(),
        }
    }

    #[napi(js_name = "view")]
    pub fn view(&mut self, dom: &Dom) -> Result<String> {
        let dom = lock_dom(&dom.inner)?;
        if dom.root_ref().is_none() {
            return Err(invalid_arg("empty DOM has no view"));
        }
        serde_json::to_string(&self.inner.view(&dom))
            .map_err(|error| upstream_error("serializing DOM view", error))
    }

    #[napi(js_name = "viewChildren")]
    pub fn view_children(&mut self, dom: &Dom) -> Result<String> {
        let dom = lock_dom(&dom.inner)?;
        if dom.root_ref().is_none() {
            return Err(invalid_arg("empty DOM has no view"));
        }
        serde_json::to_string(&self.inner.view_children(&dom))
            .map_err(|error| upstream_error("serializing DOM children view", error))
    }
}

#[napi]
pub struct Dom {
    pub(crate) inner: Arc<Mutex<WeakDom>>,
    pub(crate) source_referents: BTreeMap<String, String>,
}

#[napi]
impl Dom {
    #[napi(constructor)]
    pub fn new(spec_json: String) -> Result<Self> {
        catch_panic("WeakDom::new", || {
            Ok(Self {
                inner: Arc::new(Mutex::new(dom_from_spec_json(&spec_json)?)),
                source_referents: BTreeMap::new(),
            })
        })
    }

    #[napi(js_name = "fromRaw")]
    pub fn from_raw(raw_json: String) -> Result<Self> {
        catch_panic("WeakDom::from_raw", || {
            Ok(Self {
                inner: Arc::new(Mutex::new(dom_from_raw_json(&raw_json)?)),
                source_referents: BTreeMap::new(),
            })
        })
    }

    #[napi(js_name = "snapshot")]
    pub fn snapshot(&self) -> Result<String> {
        let dom = lock_dom(&self.inner)?;
        snapshot_json(&dom)
    }

    #[napi(js_name = "rootRef")]
    pub fn root_ref(&self) -> Result<String> {
        Ok(ref_string(lock_dom(&self.inner)?.root_ref()))
    }

    #[napi(js_name = "sourceReferents")]
    pub fn source_referents(&self) -> Result<String> {
        serde_json::to_string(&self.source_referents)
            .map_err(|error| upstream_error("serializing source referents", error))
    }

    #[napi(js_name = "root")]
    pub fn root(&self) -> Result<String> {
        let dom = lock_dom(&self.inner)?;
        if dom.root_ref().is_none() {
            return Err(invalid_arg("empty DOM has no root instance"));
        }
        serde_json::to_string(&instance_view(dom.root())?)
            .map_err(|error| upstream_error("serializing root instance", error))
    }

    #[napi(js_name = "rootMut")]
    pub fn root_mut(&self) -> Result<Instance> {
        let dom = lock_dom(&self.inner)?;
        if dom.root_ref().is_none() {
            return Err(invalid_arg("DOM root has an empty referent"));
        }
        Ok(Instance {
            dom: Arc::clone(&self.inner),
            referent: dom.root_ref(),
        })
    }

    #[napi(js_name = "instance")]
    pub fn instance(&self, referent: String) -> Result<Option<String>> {
        let dom = lock_dom(&self.inner)?;
        instance_json(&dom, parse_ref(&referent)?)
    }

    #[napi(js_name = "instanceObject")]
    pub fn instance_object(&self, referent: String) -> Result<Option<Instance>> {
        let referent = parse_ref(&referent)?;
        let dom = lock_dom(&self.inner)?;
        if dom.get_by_ref(referent).is_none() {
            return Ok(None);
        }
        Ok(Some(Instance {
            dom: Arc::clone(&self.inner),
            referent,
        }))
    }

    #[napi(js_name = "raw")]
    pub fn raw(&self) -> Result<String> {
        let dom = lock_dom(&self.inner)?;
        raw_json(&dom)
    }

    #[napi(js_name = "rawInstances")]
    pub fn raw_instances(&self) -> Result<String> {
        let dom = lock_dom(&self.inner)?;
        serde_json::to_string(&raw_instances_value(&dom)?)
            .map_err(|error| upstream_error("serializing raw instances", error))
    }

    #[napi(js_name = "children")]
    pub fn children(&self, referent: String) -> Result<Vec<String>> {
        let referent = parse_ref(&referent)?;
        let dom = lock_dom(&self.inner)?;
        let instance = dom
            .get_by_ref(referent)
            .ok_or_else(|| invalid_arg(format!("instance {referent} is not in this DOM")))?;
        Ok(instance
            .children()
            .iter()
            .copied()
            .map(ref_string)
            .collect())
    }

    #[napi(js_name = "descendants")]
    pub fn descendants(&self, referent: Option<String>) -> Result<String> {
        let dom = lock_dom(&self.inner)?;
        let instances = if let Some(referent) = referent {
            let referent = parse_ref(&referent)?;
            catch_panic("WeakDom::descendants_of", || {
                dom.descendants_of(referent)
                    .map(instance_view)
                    .collect::<Result<Vec<_>>>()
            })?
        } else if dom.root_ref().is_none() {
            Vec::new()
        } else {
            dom.descendants()
                .map(instance_view)
                .collect::<Result<Vec<_>>>()?
        };
        serde_json::to_string(&instances)
            .map_err(|error| upstream_error("serializing descendants", error))
    }

    #[napi(js_name = "ancestorsOf")]
    pub fn ancestors_of(&self, referent: String) -> Result<String> {
        let referent = parse_ref(&referent)?;
        let dom = lock_dom(&self.inner)?;
        let instances = catch_panic("WeakDom::ancestors_of", || {
            dom.ancestors_of(referent)
                .map(instance_view)
                .collect::<Result<Vec<_>>>()
        })?;
        serde_json::to_string(&instances)
            .map_err(|error| upstream_error("serializing ancestors", error))
    }

    #[napi(js_name = "fullPath")]
    pub fn full_path(&self, referent: String, separator: Option<String>) -> Result<String> {
        let referent = parse_ref(&referent)?;
        let dom = lock_dom(&self.inner)?;
        catch_panic("WeakDom::full_path_of", || {
            Ok(dom.full_path_of(referent, separator.as_deref().unwrap_or(".")))
        })
    }

    #[napi(js_name = "getProperty")]
    pub fn get_property(&self, referent: String, property: String) -> Result<Option<String>> {
        let referent = parse_ref(&referent)?;
        let dom = lock_dom(&self.inner)?;
        let Some(instance) = dom.get_by_ref(referent) else {
            return Ok(None);
        };
        instance
            .properties
            .get(&ustr(&property))
            .map(|value| {
                serde_json::to_string(&variant_to_value(value)?)
                    .map_err(|error| upstream_error("serializing property", error))
            })
            .transpose()
    }

    #[napi(js_name = "uniqueId")]
    pub fn unique_id(&self, referent: String) -> Result<Option<String>> {
        let referent = parse_ref(&referent)?;
        Ok(lock_dom(&self.inner)?
            .get_unique_id(referent)
            .map(|value| value.to_string()))
    }

    #[napi(js_name = "setProperty")]
    pub fn set_property(
        &self,
        referent: String,
        property: String,
        value_json: String,
    ) -> Result<()> {
        if property == "Name" || property == "ClassName" {
            return Err(invalid_arg(
                "Name and ClassName are instance fields; use setName or setClass",
            ));
        }
        let referent = parse_ref(&referent)?;
        let value: Value = serde_json::from_str(&value_json)
            .map_err(|error| invalid_arg(format!("invalid property JSON: {error}")))?;
        let value = parse_variant(value)?;
        let mut dom = lock_dom(&self.inner)?;
        let instance = dom
            .get_by_ref_mut(referent)
            .ok_or_else(|| invalid_arg(format!("instance {referent} is not in this DOM")))?;
        instance.properties.insert(ustr(&property), value);
        Ok(())
    }

    #[napi(js_name = "removeProperty")]
    pub fn remove_property(&self, referent: String, property: String) -> Result<bool> {
        if property == "Name" || property == "ClassName" {
            return Err(invalid_arg(
                "Name and ClassName are instance fields; use setName or setClass",
            ));
        }
        let referent = parse_ref(&referent)?;
        let mut dom = lock_dom(&self.inner)?;
        let instance = dom
            .get_by_ref_mut(referent)
            .ok_or_else(|| invalid_arg(format!("instance {referent} is not in this DOM")))?;
        Ok(instance.properties.remove(&ustr(&property)).is_some())
    }

    #[napi(js_name = "setName")]
    pub fn set_name(&self, referent: String, name: String) -> Result<()> {
        let referent = parse_ref(&referent)?;
        let mut dom = lock_dom(&self.inner)?;
        let instance = dom
            .get_by_ref_mut(referent)
            .ok_or_else(|| invalid_arg(format!("instance {referent} is not in this DOM")))?;
        instance.name = name;
        Ok(())
    }

    #[napi(js_name = "setClass")]
    pub fn set_class(&self, referent: String, class_name: String) -> Result<()> {
        let referent = parse_ref(&referent)?;
        let mut dom = lock_dom(&self.inner)?;
        let instance = dom
            .get_by_ref_mut(referent)
            .ok_or_else(|| invalid_arg(format!("instance {referent} is not in this DOM")))?;
        instance.class = ustr(&class_name);
        Ok(())
    }

    #[napi(js_name = "insert")]
    pub fn insert(&self, parent: String, spec_json: String) -> Result<String> {
        let parent = parse_ref(&parent)?;
        let (builder, referents) = build_instance(
            serde_json::from_str(&spec_json)
                .map_err(|error| invalid_arg(format!("invalid instance JSON: {error}")))?,
        )?;
        let mut dom = lock_dom(&self.inner)?;
        validate_insert(&dom, parent, &referents)?;
        let referent = catch_panic("WeakDom::insert", || Ok(dom.insert(parent, builder)))?;
        Ok(ref_string(referent))
    }

    #[napi(js_name = "insertBuilder")]
    pub fn insert_builder(&self, parent: String, builder: &mut InstanceBuilder) -> Result<String> {
        let parent = parse_ref(&parent)?;
        let referents = builder.referents.clone();
        let mut dom = lock_dom(&self.inner)?;
        validate_insert(&dom, parent, &referents)?;
        let builder = builder
            .inner
            .take()
            .ok_or_else(|| invalid_arg("InstanceBuilder has already been consumed"))?;
        let referent = catch_panic("WeakDom::insert", || Ok(dom.insert(parent, builder)))?;
        Ok(ref_string(referent))
    }

    #[napi(js_name = "reserve")]
    pub fn reserve(&self, additional: u32) -> Result<()> {
        lock_dom(&self.inner)?.reserve(additional as usize);
        Ok(())
    }

    #[napi(js_name = "destroy")]
    pub fn destroy(&self, referent: String) -> Result<()> {
        let referent = parse_ref(&referent)?;
        let mut dom = lock_dom(&self.inner)?;
        catch_panic("WeakDom::destroy", || {
            dom.destroy(referent);
            Ok(())
        })
    }

    #[napi(js_name = "cloneWithin")]
    pub fn clone_within(&self, referent: String) -> Result<String> {
        let referent = parse_ref(&referent)?;
        let mut dom = lock_dom(&self.inner)?;
        let clone = catch_panic("WeakDom::clone_within", || Ok(dom.clone_within(referent)))?;
        Ok(ref_string(clone))
    }

    #[napi(js_name = "transferWithin")]
    pub fn transfer_within(&self, referent: String, parent: String) -> Result<()> {
        let referent = parse_ref(&referent)?;
        let parent = parse_ref(&parent)?;
        let mut dom = lock_dom(&self.inner)?;
        validate_transfer(&dom, referent, parent)?;
        catch_panic("WeakDom::transfer_within", || {
            dom.transfer_within(referent, parent);
            Ok(())
        })
    }

    #[napi(js_name = "transfer")]
    pub fn transfer(&self, referent: String, destination: &Dom, parent: String) -> Result<()> {
        if Arc::ptr_eq(&self.inner, &destination.inner) {
            return Err(invalid_arg(
                "use transferWithin when source and destination are the same DOM",
            ));
        }
        let referent = parse_ref(&referent)?;
        let parent = parse_ref(&parent)?;
        let mut source = lock_dom(&self.inner)?;
        let mut dest = lock_dom(&destination.inner)?;
        validate_transfer(&source, referent, Ref::none())?;
        validate_insert(&dest, parent, &subtree_refs(&source, referent))?;
        catch_panic("WeakDom::transfer", || {
            source.transfer(referent, &mut dest, parent);
            Ok(())
        })
    }

    #[napi(js_name = "cloneIntoExternal")]
    pub fn clone_into_external(&self, referent: String, destination: &Dom) -> Result<String> {
        if Arc::ptr_eq(&self.inner, &destination.inner) {
            return Err(invalid_arg(
                "use cloneWithin when source and destination are the same DOM",
            ));
        }
        let referent = parse_ref(&referent)?;
        let source = lock_dom(&self.inner)?;
        let mut dest = lock_dom(&destination.inner)?;
        if source.get_by_ref(referent).is_none() {
            return Err(invalid_arg(format!(
                "instance {referent} is not in this DOM"
            )));
        }
        let clone = catch_panic("WeakDom::clone_into_external", || {
            Ok(source.clone_into_external(referent, &mut dest))
        })?;
        Ok(ref_string(clone))
    }

    #[napi(js_name = "cloneMultipleIntoExternal")]
    pub fn clone_multiple_into_external(
        &self,
        referents: Vec<String>,
        destination: &Dom,
    ) -> Result<Vec<String>> {
        if Arc::ptr_eq(&self.inner, &destination.inner) {
            return Err(invalid_arg("use cloneWithin for cloning into the same DOM"));
        }
        let referents = referents
            .iter()
            .map(|referent| parse_ref(referent))
            .collect::<Result<Vec<_>>>()?;
        let source = lock_dom(&self.inner)?;
        let mut dest = lock_dom(&destination.inner)?;
        for referent in &referents {
            if source.get_by_ref(*referent).is_none() {
                return Err(invalid_arg(format!(
                    "instance {referent} is not in this DOM"
                )));
            }
        }
        let clones = catch_panic("WeakDom::clone_multiple_into_external", || {
            Ok(source.clone_multiple_into_external(&referents, &mut dest))
        })?;
        Ok(clones.into_iter().map(ref_string).collect())
    }

    #[napi(js_name = "view")]
    pub fn view(&self) -> Result<String> {
        let mut viewer = UpstreamDomViewer::new();
        let dom = lock_dom(&self.inner)?;
        if dom.root_ref().is_none() {
            return Err(invalid_arg("empty DOM has no view"));
        }
        catch_panic("WeakDom::view", || {
            serde_json::to_string(&viewer.view(&dom))
                .map_err(|error| upstream_error("serializing DOM view", error))
        })
    }

    #[napi(js_name = "toXml")]
    pub fn to_xml(&self, options_json: Option<String>) -> Result<Buffer> {
        let options = parse_io_options(options_json.as_deref())?;
        let dom = lock_dom(&self.inner)?;
        Ok(Buffer::from(encode_xml_bytes(&dom, &options)?))
    }

    #[napi(js_name = "toBinary")]
    pub fn to_binary(&self, options_json: Option<String>) -> Result<Buffer> {
        let options = parse_io_options(options_json.as_deref())?;
        let dom = lock_dom(&self.inner)?;
        Ok(Buffer::from(encode_binary_bytes(&dom, &options)?))
    }

    #[napi(js_name = "toXmlWithDatabase")]
    pub fn to_xml_with_database(
        &self,
        database: &ReflectionDatabase,
        options_json: Option<String>,
    ) -> Result<Buffer> {
        let options = parse_io_options(options_json.as_deref())?;
        let database = database.parsed()?;
        let dom = lock_dom(&self.inner)?;
        Ok(Buffer::from(encode_xml_with_database(
            &dom, &options, &database,
        )?))
    }

    #[napi(js_name = "toBinaryWithDatabase")]
    pub fn to_binary_with_database(
        &self,
        database: &ReflectionDatabase,
        options_json: Option<String>,
    ) -> Result<Buffer> {
        let options = parse_io_options(options_json.as_deref())?;
        let compression = parse_compression(options.compression.as_deref())?;
        let database = database.parsed()?;
        let dom = lock_dom(&self.inner)?;
        Ok(Buffer::from(encode_binary_with_database(
            &dom,
            &options,
            compression,
            &database,
        )?))
    }

    #[napi(js_name = "instanceCount")]
    pub fn instance_count(&self) -> Result<u32> {
        Ok(lock_dom(&self.inner)?.descendants().count() as u32)
    }
}

#[napi(js_name = "createDom")]
pub fn create_dom(spec_json: String) -> Result<Dom> {
    Dom::new(spec_json)
}

#[napi(js_name = "createDomFromRaw")]
pub fn create_dom_from_raw(raw_json: String) -> Result<Dom> {
    Dom::from_raw(raw_json)
}

#[napi(js_name = "readXml")]
pub fn read_xml(data: Buffer, options_json: Option<String>) -> Result<Dom> {
    let options = parse_io_options(options_json.as_deref())?;
    let decoded = decode_xml_bytes(data.to_vec(), &options)?;
    Ok(Dom {
        inner: Arc::new(Mutex::new(decoded.dom)),
        source_referents: decoded.source_referents,
    })
}

#[napi(js_name = "readXmlWithDatabase")]
pub fn read_xml_with_database(
    data: Buffer,
    database: &ReflectionDatabase,
    options_json: Option<String>,
) -> Result<Dom> {
    let options = parse_io_options(options_json.as_deref())?;
    let database = database.parsed()?;
    let decoded = decode_xml_with_database(data.to_vec(), &options, &database)?;
    Ok(Dom {
        inner: Arc::new(Mutex::new(decoded.dom)),
        source_referents: decoded.source_referents,
    })
}

#[napi(js_name = "readBinary")]
pub fn read_binary(data: Buffer, options_json: Option<String>) -> Result<Dom> {
    let options = parse_io_options(options_json.as_deref())?;
    Ok(Dom {
        inner: Arc::new(Mutex::new(decode_binary_bytes(data.to_vec(), &options)?)),
        source_referents: BTreeMap::new(),
    })
}

#[napi(js_name = "readBinaryWithDatabase")]
pub fn read_binary_with_database(
    data: Buffer,
    database: &ReflectionDatabase,
    options_json: Option<String>,
) -> Result<Dom> {
    let _options = parse_io_options(options_json.as_deref())?;
    let database = database.parsed()?;
    Ok(Dom {
        inner: Arc::new(Mutex::new(decode_binary_with_database(
            data.to_vec(),
            &database,
        )?)),
        source_referents: BTreeMap::new(),
    })
}

#[napi(js_name = "convertFile")]
pub fn convert_file(
    data: Buffer,
    from_format: String,
    to_format: String,
    options_json: Option<String>,
) -> Result<Buffer> {
    let options = parse_io_options(options_json.as_deref())?;
    let from_format = from_format.to_ascii_lowercase();
    let to_format = to_format.to_ascii_lowercase();
    let mut decode_options = options.clone();
    if decode_options.property_behavior.is_none()
        && matches!(from_format.as_str(), "xml" | "rbxmx" | "rbxlx")
    {
        decode_options.property_behavior = Some("readUnknown".to_owned());
    }
    let mut encode_options = options;
    if encode_options.property_behavior.is_none()
        && matches!(to_format.as_str(), "xml" | "rbxmx" | "rbxlx")
    {
        encode_options.property_behavior = Some("writeUnknown".to_owned());
    }
    let dom = match from_format.as_str() {
        "xml" | "rbxmx" | "rbxlx" => decode_xml_bytes(data.to_vec(), &decode_options)?.dom,
        "binary" | "rbxm" | "rbxl" => decode_binary_bytes(data.to_vec(), &decode_options)?,
        _ => return Err(invalid_arg(format!("unknown input format {from_format:?}"))),
    };
    match to_format.as_str() {
        "xml" | "rbxmx" | "rbxlx" => Ok(Buffer::from(encode_xml_bytes(&dom, &encode_options)?)),
        "binary" | "rbxm" | "rbxl" => Ok(Buffer::from(encode_binary_bytes(&dom, &encode_options)?)),
        _ => Err(invalid_arg(format!("unknown output format {to_format:?}"))),
    }
}
