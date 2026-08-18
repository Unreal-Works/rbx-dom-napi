#![allow(dead_code)]

use std::collections::{BTreeMap, HashSet};
use std::io::Cursor;
use std::str::FromStr;

use napi::bindgen_prelude::Buffer;
use napi::Result;
use napi_derive::napi;
use rbx_binary::{CompressionType, Deserializer, Serializer};
use rbx_dom_weak::{ustr, InstanceBuilder, WeakDom};
use rbx_reflection_database::get_bundled;
use rbx_types::{Ref, Variant};
use rbx_xml::{DecodeOptions, DecodePropertyBehavior, EncodeOptions, EncodePropertyBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{catch_panic, invalid_arg, upstream_error};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub(crate) struct IoOptions {
    #[serde(rename = "propertyBehavior", alias = "property_behavior")]
    pub(crate) property_behavior: Option<String>,
    pub(crate) compression: Option<String>,
    #[serde(rename = "includeRoot", alias = "include_root")]
    pub(crate) include_root: bool,
    pub(crate) refs: Option<Vec<String>>,
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

#[derive(Debug, Serialize)]
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

fn build_instance(spec: InstanceSpec) -> Result<(InstanceBuilder, Vec<Ref>)> {
    let class_name = spec.class_name.unwrap_or_else(|| "Folder".to_owned());
    let mut builder = InstanceBuilder::new(ustr(&class_name));

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
    Ok(DomSnapshot {
        root_ref: ref_string(dom.root_ref()),
        instances: dom
            .descendants()
            .map(instance_view)
            .collect::<Result<Vec<_>>>()?,
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

pub(crate) fn decode_xml_bytes(data: Vec<u8>, options: &IoOptions) -> Result<WeakDom> {
    catch_panic("rbx_xml::from_reader", || {
        let mut decoder = DecodeOptions::new().reflection_database(get_bundled());
        if let Some(behavior) = parse_property_behavior(options.property_behavior.as_deref())? {
            decoder = decoder.property_behavior(behavior);
        }
        rbx_xml::from_reader(Cursor::new(data), decoder)
            .map_err(|error| upstream_error("rbx_xml::from_reader", error))
    })
}

pub(crate) fn encode_xml_bytes(dom: &WeakDom, options: &IoOptions) -> Result<Vec<u8>> {
    catch_panic("rbx_xml::to_writer", || {
        let mut encoder = EncodeOptions::new().reflection_database(get_bundled());
        if let Some(behavior) =
            parse_encode_property_behavior(options.property_behavior.as_deref())?
        {
            encoder = encoder.property_behavior(behavior);
        }
        let refs = refs_for(dom, options)?;
        let mut output = Vec::new();
        rbx_xml::to_writer(&mut output, dom, &refs, encoder)
            .map_err(|error| upstream_error("rbx_xml::to_writer", error))?;
        Ok(output)
    })
}

pub(crate) fn decode_binary_bytes(data: Vec<u8>) -> Result<WeakDom> {
    catch_panic("rbx_binary::from_reader", || {
        let decoder = Deserializer::new().reflection_database(get_bundled());
        decoder
            .deserialize(Cursor::new(data))
            .map_err(|error| upstream_error("rbx_binary::from_reader", error))
    })
}

pub(crate) fn encode_binary_bytes(dom: &WeakDom, options: &IoOptions) -> Result<Vec<u8>> {
    catch_panic("rbx_binary::to_writer", || {
        let compression = parse_compression(options.compression.as_deref())?;
        let encoder = Serializer::new()
            .reflection_database(get_bundled())
            .compression_type(compression);
        let refs = refs_for(dom, options)?;
        let mut output = Vec::new();
        encoder
            .serialize(&mut output, dom, &refs)
            .map_err(|error| upstream_error("rbx_binary::to_writer", error))?;
        Ok(output)
    })
}

#[napi]
pub struct Dom {
    pub(crate) inner: WeakDom,
}

#[napi]
impl Dom {
    #[napi(constructor)]
    pub fn new(spec_json: String) -> Result<Self> {
        catch_panic("WeakDom::new", || {
            Ok(Self {
                inner: dom_from_spec_json(&spec_json)?,
            })
        })
    }

    #[napi(js_name = "snapshot")]
    pub fn snapshot(&self) -> Result<String> {
        snapshot_json(&self.inner)
    }

    #[napi(js_name = "rootRef")]
    pub fn root_ref(&self) -> String {
        ref_string(self.inner.root_ref())
    }

    #[napi(js_name = "instance")]
    pub fn instance(&self, referent: String) -> Result<Option<String>> {
        instance_json(&self.inner, parse_ref(&referent)?)
    }

    #[napi(js_name = "children")]
    pub fn children(&self, referent: String) -> Result<Vec<String>> {
        let referent = parse_ref(&referent)?;
        let instance = self
            .inner
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
        let instances = if let Some(referent) = referent {
            let referent = parse_ref(&referent)?;
            catch_panic("WeakDom::descendants_of", || {
                self.inner
                    .descendants_of(referent)
                    .map(instance_view)
                    .collect::<Result<Vec<_>>>()
            })?
        } else {
            self.inner
                .descendants()
                .map(instance_view)
                .collect::<Result<Vec<_>>>()?
        };
        serde_json::to_string(&instances)
            .map_err(|error| upstream_error("serializing descendants", error))
    }

    #[napi(js_name = "fullPath")]
    pub fn full_path(&self, referent: String, separator: Option<String>) -> Result<String> {
        let referent = parse_ref(&referent)?;
        catch_panic("WeakDom::full_path_of", || {
            Ok(self
                .inner
                .full_path_of(referent, separator.as_deref().unwrap_or(".")))
        })
    }

    #[napi(js_name = "getProperty")]
    pub fn get_property(&self, referent: String, property: String) -> Result<Option<String>> {
        let referent = parse_ref(&referent)?;
        let Some(instance) = self.inner.get_by_ref(referent) else {
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

    #[napi(js_name = "setProperty")]
    pub fn set_property(
        &mut self,
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
        let instance = self
            .inner
            .get_by_ref_mut(referent)
            .ok_or_else(|| invalid_arg(format!("instance {referent} is not in this DOM")))?;
        instance.properties.insert(ustr(&property), value);
        Ok(())
    }

    #[napi(js_name = "removeProperty")]
    pub fn remove_property(&mut self, referent: String, property: String) -> Result<bool> {
        if property == "Name" || property == "ClassName" {
            return Err(invalid_arg(
                "Name and ClassName are instance fields; use setName or setClass",
            ));
        }
        let referent = parse_ref(&referent)?;
        let instance = self
            .inner
            .get_by_ref_mut(referent)
            .ok_or_else(|| invalid_arg(format!("instance {referent} is not in this DOM")))?;
        Ok(instance.properties.remove(&ustr(&property)).is_some())
    }

    #[napi(js_name = "setName")]
    pub fn set_name(&mut self, referent: String, name: String) -> Result<()> {
        let referent = parse_ref(&referent)?;
        let instance = self
            .inner
            .get_by_ref_mut(referent)
            .ok_or_else(|| invalid_arg(format!("instance {referent} is not in this DOM")))?;
        instance.name = name;
        Ok(())
    }

    #[napi(js_name = "setClass")]
    pub fn set_class(&mut self, referent: String, class_name: String) -> Result<()> {
        let referent = parse_ref(&referent)?;
        let instance = self
            .inner
            .get_by_ref_mut(referent)
            .ok_or_else(|| invalid_arg(format!("instance {referent} is not in this DOM")))?;
        instance.class = ustr(&class_name);
        Ok(())
    }

    #[napi(js_name = "insert")]
    pub fn insert(&mut self, parent: String, spec_json: String) -> Result<String> {
        let parent = parse_ref(&parent)?;
        let (builder, referents) = build_instance(
            serde_json::from_str(&spec_json)
                .map_err(|error| invalid_arg(format!("invalid instance JSON: {error}")))?,
        )?;
        validate_insert(&self.inner, parent, &referents)?;
        let referent = catch_panic("WeakDom::insert", || Ok(self.inner.insert(parent, builder)))?;
        Ok(ref_string(referent))
    }

    #[napi(js_name = "destroy")]
    pub fn destroy(&mut self, referent: String) -> Result<()> {
        let referent = parse_ref(&referent)?;
        catch_panic("WeakDom::destroy", || {
            self.inner.destroy(referent);
            Ok(())
        })
    }

    #[napi(js_name = "cloneWithin")]
    pub fn clone_within(&mut self, referent: String) -> Result<String> {
        let referent = parse_ref(&referent)?;
        let clone = catch_panic("WeakDom::clone_within", || {
            Ok(self.inner.clone_within(referent))
        })?;
        Ok(ref_string(clone))
    }

    #[napi(js_name = "transferWithin")]
    pub fn transfer_within(&mut self, referent: String, parent: String) -> Result<()> {
        let referent = parse_ref(&referent)?;
        let parent = parse_ref(&parent)?;
        validate_transfer(&self.inner, referent, parent)?;
        catch_panic("WeakDom::transfer_within", || {
            self.inner.transfer_within(referent, parent);
            Ok(())
        })
    }

    #[napi(js_name = "toXml")]
    pub fn to_xml(&self, options_json: Option<String>) -> Result<Buffer> {
        let options = parse_io_options(options_json.as_deref())?;
        Ok(Buffer::from(encode_xml_bytes(&self.inner, &options)?))
    }

    #[napi(js_name = "toBinary")]
    pub fn to_binary(&self, options_json: Option<String>) -> Result<Buffer> {
        let options = parse_io_options(options_json.as_deref())?;
        Ok(Buffer::from(encode_binary_bytes(&self.inner, &options)?))
    }

    #[napi(js_name = "instanceCount")]
    pub fn instance_count(&self) -> u32 {
        self.inner.descendants().count() as u32
    }
}

#[napi(js_name = "createDom")]
pub fn create_dom(spec_json: String) -> Result<Dom> {
    Dom::new(spec_json)
}

#[napi(js_name = "readXml")]
pub fn read_xml(data: Buffer, options_json: Option<String>) -> Result<Dom> {
    let options = parse_io_options(options_json.as_deref())?;
    Ok(Dom {
        inner: decode_xml_bytes(data.to_vec(), &options)?,
    })
}

#[napi(js_name = "readBinary")]
pub fn read_binary(data: Buffer) -> Result<Dom> {
    Ok(Dom {
        inner: decode_binary_bytes(data.to_vec())?,
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
        "xml" | "rbxmx" | "rbxlx" => decode_xml_bytes(data.to_vec(), &decode_options)?,
        "binary" | "rbxm" | "rbxl" => decode_binary_bytes(data.to_vec())?,
        _ => return Err(invalid_arg(format!("unknown input format {from_format:?}"))),
    };
    match to_format.as_str() {
        "xml" | "rbxmx" | "rbxlx" => Ok(Buffer::from(encode_xml_bytes(&dom, &encode_options)?)),
        "binary" | "rbxm" | "rbxl" => Ok(Buffer::from(encode_binary_bytes(&dom, &encode_options)?)),
        _ => Err(invalid_arg(format!("unknown output format {to_format:?}"))),
    }
}
