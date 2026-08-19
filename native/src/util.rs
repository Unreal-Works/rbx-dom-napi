#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

use napi::bindgen_prelude::Buffer;
use napi::Result;
use napi_derive::napi;
use rbx_dom_weak::ustr;
use serde_json::to_string_pretty;

use crate::dom::{
    decode_binary_bytes, decode_xml_bytes, encode_binary_bytes, encode_xml_bytes, Dom, IoOptions,
};
use crate::error::{catch_panic, invalid_arg, upstream_error};

#[napi(js_name = "viewBinary")]
pub fn view_binary(data: Buffer) -> Result<String> {
    catch_panic("rbx_binary::text_format", || {
        let model = rbx_binary::text_format::DecodedModel::from_reader(Cursor::new(data.to_vec()));
        to_string_pretty(&model).map_err(|error| upstream_error("serializing binary view", error))
    })
}

#[napi(js_name = "viewBinaryText")]
pub fn view_binary_text(data: Buffer) -> Result<String> {
    catch_panic("rbx_binary::text_format", || {
        let model = rbx_binary::text_format::DecodedModel::from_reader(Cursor::new(data.to_vec()));
        yaml_serde::to_string(&model)
            .map_err(|error| upstream_error("serializing binary text view", error))
    })
}

#[napi(js_name = "removeProperty")]
pub fn remove_property(
    data: Buffer,
    format: String,
    class_name: String,
    property_name: String,
    output_format: Option<String>,
) -> Result<Buffer> {
    let decode_options = IoOptions {
        property_behavior: Some("readUnknown".to_owned()),
        ..IoOptions::default()
    };
    let encode_options = IoOptions {
        property_behavior: Some("writeUnknown".to_owned()),
        ..IoOptions::default()
    };
    let format = format.to_ascii_lowercase();
    let output_format = output_format
        .unwrap_or_else(|| format.clone())
        .to_ascii_lowercase();
    let dom = Dom {
        inner: match format.as_str() {
            "xml" | "rbxmx" | "rbxlx" => Arc::new(Mutex::new(
                decode_xml_bytes(data.to_vec(), &decode_options)?.dom,
            )),
            "binary" | "rbxm" | "rbxl" => Arc::new(Mutex::new(decode_binary_bytes(
                data.to_vec(),
                &decode_options,
            )?)),
            _ => return Err(invalid_arg(format!("unknown input format {format:?}"))),
        },
        xml_version: None,
        source_referents: BTreeMap::new(),
    };

    let mut inner = dom
        .inner
        .lock()
        .map_err(|_| crate::error::failure("DOM lock is poisoned"))?;
    let targets: Vec<_> = inner
        .descendants()
        .filter(|instance| instance.class == class_name.as_str())
        .map(|instance| instance.referent())
        .collect();
    for referent in targets {
        if let Some(instance) = inner.get_by_ref_mut(referent) {
            instance.properties.remove(&ustr(&property_name));
        }
    }

    match output_format.as_str() {
        "xml" | "rbxmx" | "rbxlx" => Ok(Buffer::from(encode_xml_bytes(&inner, &encode_options)?)),
        "binary" | "rbxm" | "rbxl" => {
            Ok(Buffer::from(encode_binary_bytes(&inner, &encode_options)?))
        }
        _ => Err(invalid_arg(format!(
            "unknown output format {output_format:?}"
        ))),
    }
}
