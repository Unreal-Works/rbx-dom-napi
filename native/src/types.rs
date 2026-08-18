#![allow(dead_code)]

use std::str::FromStr;

use napi::bindgen_prelude::Buffer;
use napi::Result;
use napi_derive::napi;
use rbx_types::{
    Attributes as UpstreamAttributes, Axes, BinaryString, BrickColor, CFrame, Color3, Color3uint8,
    ColorSequence, ColorSequenceKeypoint, Content, ContentId, CustomPhysicalProperties, Enum,
    EnumItem, Faces, Font, FontStyle, FontWeight, MaterialColors, Matrix3, NetAssetRef,
    NumberRange, NumberSequence, NumberSequenceKeypoint, PhysicalProperties, Ray, Rect, Ref,
    Region3, Region3int16, SecurityCapabilities, SharedString, Tags, TerrainMaterials, UDim, UDim2,
    UniqueId, Vector2, Vector2int16, Vector3, Vector3int16,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::dom::{parse_variant, ref_string, variant_to_value};
use crate::error::{invalid_arg, upstream_error};

fn from_json<T: DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value)
        .map_err(|error| invalid_arg(format!("invalid upstream type JSON: {error}")))
}

fn to_json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|error| upstream_error("serializing upstream type", error))
}

fn checked_i16(value: i32, name: &str) -> Result<i16> {
    i16::try_from(value).map_err(|_| invalid_arg(format!("{name} must fit in an i16")))
}

fn checked_u8(value: u32, name: &str) -> Result<u8> {
    u8::try_from(value).map_err(|_| invalid_arg(format!("{name} must fit in a u8")))
}

fn checked_u16(value: u32, name: &str) -> Result<u16> {
    u16::try_from(value).map_err(|_| invalid_arg(format!("{name} must fit in a u16")))
}

#[napi(js_name = "vector2")]
pub fn vector2(x: f64, y: f64) -> Result<String> {
    to_json(&Vector2::new(x as f32, y as f32))
}

#[napi(js_name = "vector2int16")]
pub fn vector2int16(x: i32, y: i32) -> Result<String> {
    to_json(&Vector2int16::new(
        checked_i16(x, "x")?,
        checked_i16(y, "y")?,
    ))
}

#[napi(js_name = "vector3")]
pub fn vector3(x: f64, y: f64, z: f64) -> Result<String> {
    to_json(&Vector3::new(x as f32, y as f32, z as f32))
}

#[napi(js_name = "vector3int16")]
pub fn vector3int16(x: i32, y: i32, z: i32) -> Result<String> {
    to_json(&Vector3int16::new(
        checked_i16(x, "x")?,
        checked_i16(y, "y")?,
        checked_i16(z, "z")?,
    ))
}

#[napi(js_name = "vector3NormalId")]
pub fn vector3_normal_id(value_json: String) -> Result<Option<u8>> {
    let value: Vector3 = from_json(&value_json)?;
    Ok(value.to_normal_id())
}

#[napi(js_name = "color3")]
pub fn color3(r: f64, g: f64, b: f64) -> Result<String> {
    to_json(&Color3::new(r as f32, g as f32, b as f32))
}

#[napi(js_name = "color3uint8")]
pub fn color3uint8(r: u32, g: u32, b: u32) -> Result<String> {
    to_json(&Color3uint8::new(
        checked_u8(r, "r")?,
        checked_u8(g, "g")?,
        checked_u8(b, "b")?,
    ))
}

#[napi(js_name = "cframeIdentity")]
pub fn cframe_identity() -> Result<String> {
    to_json(&CFrame::identity())
}

#[napi(js_name = "cframeFromPosition")]
pub fn cframe_from_position(position_json: String) -> Result<String> {
    let position: Vector3 = from_json(&position_json)?;
    to_json(&CFrame::new(position, Matrix3::identity()))
}

#[napi(js_name = "cframeFromMatrix")]
pub fn cframe_from_matrix(
    position_json: String,
    x_json: String,
    y_json: String,
    z_json: String,
) -> Result<String> {
    let position: Vector3 = from_json(&position_json)?;
    let x: Vector3 = from_json(&x_json)?;
    let y: Vector3 = from_json(&y_json)?;
    let z: Vector3 = from_json(&z_json)?;
    to_json(&CFrame::new(position, Matrix3::new(x, y, z)))
}

#[napi(js_name = "ray")]
pub fn ray(origin_json: String, direction_json: String) -> Result<String> {
    let origin: Vector3 = from_json(&origin_json)?;
    let direction: Vector3 = from_json(&direction_json)?;
    to_json(&Ray::new(origin, direction))
}

#[napi(js_name = "region3")]
pub fn region3(min_json: String, max_json: String) -> Result<String> {
    let min: Vector3 = from_json(&min_json)?;
    let max: Vector3 = from_json(&max_json)?;
    to_json(&Region3::new(min, max))
}

#[napi(js_name = "region3int16")]
pub fn region3int16(min_json: String, max_json: String) -> Result<String> {
    let min: Vector3int16 = from_json(&min_json)?;
    let max: Vector3int16 = from_json(&max_json)?;
    to_json(&Region3int16::new(min, max))
}

#[napi(js_name = "rect")]
pub fn rect(min_json: String, max_json: String) -> Result<String> {
    let min: Vector2 = from_json(&min_json)?;
    let max: Vector2 = from_json(&max_json)?;
    to_json(&Rect::new(min, max))
}

#[napi(js_name = "udim")]
pub fn udim(scale: f64, offset: i32) -> Result<String> {
    to_json(&UDim::new(scale as f32, offset))
}

#[napi(js_name = "udim2")]
pub fn udim2(x_json: String, y_json: String) -> Result<String> {
    let x: UDim = from_json(&x_json)?;
    let y: UDim = from_json(&y_json)?;
    to_json(&UDim2::new(x, y))
}

#[napi(js_name = "numberRange")]
pub fn number_range(min: f64, max: Option<f64>) -> Result<String> {
    let min = min as f32;
    let max = max.unwrap_or(min as f64) as f32;
    to_json(&NumberRange::new(min, max))
}

#[napi(js_name = "numberSequenceKeypoint")]
pub fn number_sequence_keypoint(time: f64, value: f64, envelope: f64) -> Result<String> {
    to_json(&NumberSequenceKeypoint::new(
        time as f32,
        value as f32,
        envelope as f32,
    ))
}

#[napi(js_name = "numberSequence")]
pub fn number_sequence(keypoints_json: String) -> Result<String> {
    let keypoints: Vec<NumberSequenceKeypoint> = from_json(&keypoints_json)?;
    to_json(&NumberSequence { keypoints })
}

#[napi(js_name = "colorSequenceKeypoint")]
pub fn color_sequence_keypoint(time: f64, color_json: String) -> Result<String> {
    let color: Color3 = from_json(&color_json)?;
    to_json(&ColorSequenceKeypoint::new(time as f32, color))
}

#[napi(js_name = "colorSequence")]
pub fn color_sequence(keypoints_json: String) -> Result<String> {
    let keypoints: Vec<ColorSequenceKeypoint> = from_json(&keypoints_json)?;
    to_json(&ColorSequence { keypoints })
}

#[napi(js_name = "font")]
pub fn font(family: String, weight: u32, style: u32) -> Result<String> {
    let weight = FontWeight::from_u16(checked_u16(weight, "weight")?)
        .ok_or_else(|| invalid_arg(format!("unsupported FontWeight {weight}")))?;
    let style = FontStyle::from_u8(checked_u8(style, "style")?)
        .ok_or_else(|| invalid_arg(format!("unsupported FontStyle {style}")))?;
    to_json(&Font::new(&family, weight, style))
}

#[napi(js_name = "fontRegular")]
pub fn font_regular(family: String) -> Result<String> {
    to_json(&Font::regular(&family))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FontParts {
    family: String,
    weight: u16,
    style: u8,
    cached_face_id: Option<String>,
}

#[napi(js_name = "fontParts")]
pub fn font_parts(value_json: String) -> Result<String> {
    let value: Font = from_json(&value_json)?;
    to_json(&FontParts {
        family: value.family,
        weight: value.weight.as_u16(),
        style: value.style.as_u8(),
        cached_face_id: value.cached_face_id,
    })
}

#[napi(js_name = "brickColorByName")]
pub fn brick_color_by_name(name: String) -> Result<String> {
    let color = BrickColor::from_name(&name)
        .ok_or_else(|| invalid_arg(format!("unknown BrickColor name {name:?}")))?;
    to_json(&color)
}

#[napi(js_name = "brickColorByNumber")]
pub fn brick_color_by_number(number: u32) -> Result<String> {
    let color = BrickColor::from_number(checked_u16(number, "number")?)
        .ok_or_else(|| invalid_arg(format!("unknown BrickColor number {number}")))?;
    to_json(&color)
}

#[napi(js_name = "brickColorToColor3uint8")]
pub fn brick_color_to_color3uint8(value_json: String) -> Result<String> {
    let value: BrickColor = from_json(&value_json)?;
    to_json(&value.to_color3uint8())
}

#[napi(js_name = "enumValue")]
pub fn enum_value(value: u32) -> u32 {
    Enum::from_u32(value).to_u32()
}

#[napi(js_name = "enumItem")]
pub fn enum_item(ty: String, value: u32) -> Result<String> {
    to_json(&EnumItem { ty, value })
}

#[napi(js_name = "refFromString")]
pub fn ref_from_string(value: String) -> Result<String> {
    Ref::from_str(&value)
        .map(ref_string)
        .map_err(|error| invalid_arg(format!("invalid Ref {value:?}: {error}")))
}

#[napi(js_name = "refNone")]
pub fn ref_none() -> String {
    ref_string(Ref::none())
}

#[napi(js_name = "refNew")]
pub fn ref_new() -> String {
    ref_string(Ref::new())
}

#[napi(js_name = "refIsSome")]
pub fn ref_is_some(value: String) -> Result<bool> {
    Ok(Ref::from_str(&value)
        .map_err(|error| invalid_arg(format!("invalid Ref {value:?}: {error}")))?
        .is_some())
}

#[napi(js_name = "refIsNone")]
pub fn ref_is_none(value: String) -> Result<bool> {
    Ok(Ref::from_str(&value)
        .map_err(|error| invalid_arg(format!("invalid Ref {value:?}: {error}")))?
        .is_none())
}

#[napi(js_name = "uniqueIdNow")]
pub fn unique_id_now() -> Result<String> {
    UniqueId::now()
        .map(|value| value.to_string())
        .map_err(|error| upstream_error("UniqueId::now", error))
}

#[napi(js_name = "uniqueIdNil")]
pub fn unique_id_nil() -> String {
    UniqueId::nil().to_string()
}

#[napi(js_name = "uniqueId")]
pub fn unique_id(index: u32, time: u32, random: String) -> Result<String> {
    let random = random
        .parse::<i64>()
        .map_err(|error| invalid_arg(format!("random must be an i64 string: {error}")))?;
    Ok(UniqueId::new(index, time, random).to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UniqueIdParts {
    index: u32,
    time: u32,
    random: String,
    nil: bool,
}

#[napi(js_name = "uniqueIdParts")]
pub fn unique_id_parts(value: String) -> Result<String> {
    let value = value
        .parse::<UniqueId>()
        .map_err(|error| invalid_arg(format!("invalid UniqueId: {error}")))?;
    to_json(&UniqueIdParts {
        index: value.index(),
        time: value.time(),
        random: value.random().to_string(),
        nil: value.is_nil(),
    })
}

#[napi(js_name = "axesFromBits")]
pub fn axes_from_bits(bits: u32) -> Result<String> {
    let value = Axes::from_bits(checked_u8(bits, "bits")?)
        .ok_or_else(|| invalid_arg(format!("invalid Axes bit mask {bits}")))?;
    to_json(&value)
}

#[napi(js_name = "axesBits")]
pub fn axes_bits(value_json: String) -> Result<u32> {
    let value: Axes = from_json(&value_json)?;
    Ok(value.bits() as u32)
}

#[napi(js_name = "axesEmpty")]
pub fn axes_empty() -> Result<String> {
    to_json(&Axes::empty())
}

#[napi(js_name = "axesAll")]
pub fn axes_all() -> Result<String> {
    to_json(&Axes::all())
}

#[napi(js_name = "axesContains")]
pub fn axes_contains(value_json: String, other_json: String) -> Result<bool> {
    Ok(from_json::<Axes>(&value_json)?.contains(from_json::<Axes>(&other_json)?))
}

#[napi(js_name = "facesFromBits")]
pub fn faces_from_bits(bits: u32) -> Result<String> {
    let value = Faces::from_bits(checked_u8(bits, "bits")?)
        .ok_or_else(|| invalid_arg(format!("invalid Faces bit mask {bits}")))?;
    to_json(&value)
}

#[napi(js_name = "facesBits")]
pub fn faces_bits(value_json: String) -> Result<u32> {
    let value: Faces = from_json(&value_json)?;
    Ok(value.bits() as u32)
}

#[napi(js_name = "facesEmpty")]
pub fn faces_empty() -> Result<String> {
    to_json(&Faces::empty())
}

#[napi(js_name = "facesAll")]
pub fn faces_all() -> Result<String> {
    to_json(&Faces::all())
}

#[napi(js_name = "facesContains")]
pub fn faces_contains(value_json: String, other_json: String) -> Result<bool> {
    Ok(from_json::<Faces>(&value_json)?.contains(from_json::<Faces>(&other_json)?))
}

#[napi(js_name = "securityCapabilitiesBits")]
pub fn security_capabilities_bits(bits: String) -> Result<String> {
    let bits = bits
        .parse::<u64>()
        .map_err(|error| invalid_arg(format!("bits must be a u64 string: {error}")))?;
    Ok(SecurityCapabilities::from_bits(bits).bits().to_string())
}

#[napi(js_name = "contentNone")]
pub fn content_none() -> Result<String> {
    to_json(&Content::none())
}

#[napi(js_name = "contentUri")]
pub fn content_uri(uri: String) -> Result<String> {
    to_json(&Content::from_uri(uri))
}

#[napi(js_name = "contentObject")]
pub fn content_object(referent: String) -> Result<String> {
    let referent = Ref::from_str(&referent)
        .map_err(|error| invalid_arg(format!("invalid Content object Ref: {error}")))?;
    to_json(&Content::from_referent(referent))
}

#[napi(js_name = "contentId")]
pub fn content_id(value: String) -> Result<String> {
    to_json(&ContentId::from(value))
}

#[napi(js_name = "physicalProperties")]
pub fn physical_properties(
    density: f64,
    friction: f64,
    elasticity: f64,
    friction_weight: f64,
    elasticity_weight: f64,
    acoustic_absorption: f64,
) -> Result<String> {
    let value = PhysicalProperties::Custom(CustomPhysicalProperties::new(
        density as f32,
        friction as f32,
        elasticity as f32,
        friction_weight as f32,
        elasticity_weight as f32,
        acoustic_absorption as f32,
    ));
    to_json(&value)
}

#[napi(js_name = "physicalPropertiesDefault")]
pub fn physical_properties_default() -> Result<String> {
    to_json(&PhysicalProperties::Default)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalPropertiesParts {
    density: f32,
    friction: f32,
    elasticity: f32,
    friction_weight: f32,
    elasticity_weight: f32,
    acoustic_absorption: f32,
}

#[napi(js_name = "physicalPropertiesParts")]
pub fn physical_properties_parts(value_json: String) -> Result<Option<String>> {
    let value: PhysicalProperties = from_json(&value_json)?;
    match value {
        PhysicalProperties::Default => Ok(None),
        PhysicalProperties::Custom(value) => to_json(&PhysicalPropertiesParts {
            density: value.density(),
            friction: value.friction(),
            elasticity: value.elasticity(),
            friction_weight: value.friction_weight(),
            elasticity_weight: value.elasticity_weight(),
            acoustic_absorption: value.acoustic_absorption(),
        })
        .map(Some),
    }
}

#[napi(js_name = "binaryString")]
pub fn binary_string(data: Buffer) -> Buffer {
    Buffer::from(BinaryString::from(data.to_vec()).into_vec())
}

#[napi(js_name = "sharedString")]
pub fn shared_string(data: Buffer) -> Result<String> {
    to_json(&SharedString::new(data.to_vec()))
}

#[napi(js_name = "sharedStringData")]
pub fn shared_string_data(value_json: String) -> Result<Buffer> {
    let value: SharedString = from_json(&value_json)?;
    Ok(Buffer::from(value.data().to_vec()))
}

#[napi(js_name = "sharedStringHashBytes")]
pub fn shared_string_hash_bytes(value_json: String) -> Result<Buffer> {
    let value: SharedString = from_json(&value_json)?;
    Ok(Buffer::from(value.hash().as_bytes().to_vec()))
}

#[napi(js_name = "sharedStringHash")]
pub fn shared_string_hash(data: Buffer) -> String {
    format!("{}", SharedString::new(data.to_vec()).hash())
}

#[napi(js_name = "netAssetRef")]
pub fn net_asset_ref(data: Buffer) -> Result<String> {
    to_json(&NetAssetRef::new(data.to_vec()))
}

#[napi(js_name = "netAssetRefData")]
pub fn net_asset_ref_data(value_json: String) -> Result<Buffer> {
    let value: NetAssetRef = from_json(&value_json)?;
    Ok(Buffer::from(value.data().to_vec()))
}

#[napi(js_name = "netAssetRefHashBytes")]
pub fn net_asset_ref_hash_bytes(value_json: String) -> Result<Buffer> {
    let value: NetAssetRef = from_json(&value_json)?;
    Ok(Buffer::from(value.hash().as_bytes().to_vec()))
}

#[napi(js_name = "netAssetRefHash")]
pub fn net_asset_ref_hash(data: Buffer) -> String {
    format!("{}", NetAssetRef::new(data.to_vec()).hash())
}

#[napi(js_name = "tagsEncode")]
pub fn tags_encode(tags_json: String) -> Result<Buffer> {
    let tags: Vec<String> = from_json(&tags_json)?;
    let mut value = Tags::new();
    for tag in tags {
        value.push(&tag);
    }
    Ok(Buffer::from(value.encode()))
}

#[napi(js_name = "tagsDecode")]
pub fn tags_decode(data: Buffer) -> Result<String> {
    let value = Tags::decode(&data).map_err(|error| upstream_error("Tags::decode", error))?;
    let tags: Vec<_> = value.iter().map(str::to_owned).collect();
    to_json(&tags)
}

#[napi(js_name = "tagsLength")]
pub fn tags_length(data: Buffer) -> Result<u32> {
    Ok(Tags::decode(&data)
        .map_err(|error| upstream_error("Tags::decode", error))?
        .len() as u32)
}

#[napi(js_name = "tagsIsEmpty")]
pub fn tags_is_empty(data: Buffer) -> Result<bool> {
    Ok(Tags::decode(&data)
        .map_err(|error| upstream_error("Tags::decode", error))?
        .is_empty())
}

#[napi(js_name = "materialColorsDefault")]
pub fn material_colors_default() -> Result<String> {
    to_json(&MaterialColors::new())
}

#[napi(js_name = "materialColorsEncode")]
pub fn material_colors_encode(value_json: String) -> Result<Buffer> {
    let value: MaterialColors = from_json(&value_json)?;
    Ok(Buffer::from(value.encode()))
}

#[napi(js_name = "materialColorsDecode")]
pub fn material_colors_decode(data: Buffer) -> Result<String> {
    let value = MaterialColors::decode(&data)
        .map_err(|error| upstream_error("MaterialColors::decode", error))?;
    to_json(&value)
}

#[napi(js_name = "variant")]
pub fn variant(value_json: String) -> Result<String> {
    let value: serde_json::Value = from_json(&value_json)?;
    let value = parse_variant(value)?;
    to_json(&variant_to_value(&value)?)
}

#[napi(js_name = "variantType")]
pub fn variant_type(value_json: String) -> Result<String> {
    let value: serde_json::Value = from_json(&value_json)?;
    let value = parse_variant(value)?;
    Ok(format!("{:?}", value.ty()))
}

#[napi]
pub struct Attributes {
    inner: UpstreamAttributes,
}

#[napi]
impl Attributes {
    #[napi(constructor)]
    pub fn new(value_json: Option<String>) -> Result<Self> {
        let inner = match value_json {
            Some(value_json) => from_json(&value_json)?,
            None => UpstreamAttributes::new(),
        };
        Ok(Self { inner })
    }

    #[napi(js_name = "get")]
    pub fn get(&self, key: String) -> Result<Option<String>> {
        self.inner
            .get(key)
            .map(|value| to_json(&variant_to_value(value)?))
            .transpose()
    }

    #[napi(js_name = "set")]
    pub fn set(&mut self, key: String, value_json: String) -> Result<Option<String>> {
        let value: Value = from_json(&value_json)?;
        let value = parse_variant(value)?;
        self.inner
            .insert(key, value)
            .map(|previous| to_json(&variant_to_value(&previous)?))
            .transpose()
    }

    #[napi(js_name = "remove")]
    pub fn remove(&mut self, key: String) -> Result<Option<String>> {
        self.inner
            .remove(key)
            .map(|value| to_json(&variant_to_value(&value)?))
            .transpose()
    }

    #[napi(js_name = "clear")]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    #[napi(js_name = "length")]
    pub fn length(&self) -> u32 {
        self.inner.len() as u32
    }

    #[napi(js_name = "isEmpty")]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[napi(js_name = "toJson")]
    pub fn to_json(&self) -> Result<String> {
        to_json(&self.inner)
    }

    #[napi(js_name = "encode")]
    pub fn encode(&self) -> Result<Buffer> {
        let mut output = Vec::new();
        self.inner
            .to_writer(&mut output)
            .map_err(|error| upstream_error("Attributes::to_writer", error))?;
        Ok(Buffer::from(output))
    }
}

#[napi(js_name = "attributesEncode")]
pub fn attributes_encode(value_json: String) -> Result<Buffer> {
    let value: UpstreamAttributes = from_json(&value_json)?;
    let mut output = Vec::new();
    value
        .to_writer(&mut output)
        .map_err(|error| upstream_error("Attributes::to_writer", error))?;
    Ok(Buffer::from(output))
}

#[napi(js_name = "attributesDecode")]
pub fn attributes_decode(data: Buffer) -> Result<Attributes> {
    Ok(Attributes {
        inner: UpstreamAttributes::from_reader(&data[..])
            .map_err(|error| upstream_error("Attributes::from_reader", error))?,
    })
}

#[napi(js_name = "contentKind")]
pub fn content_kind(value_json: String) -> Result<String> {
    let value: Content = from_json(&value_json)?;
    Ok(match value.value() {
        rbx_types::ContentType::None => "None",
        rbx_types::ContentType::Uri(_) => "Uri",
        rbx_types::ContentType::Object(_) => "Object",
        _ => "Unknown",
    }
    .to_owned())
}

#[napi(js_name = "contentUriValue")]
pub fn content_uri_value(value_json: String) -> Result<Option<String>> {
    Ok(from_json::<Content>(&value_json)?
        .as_uri()
        .map(str::to_owned))
}

#[napi(js_name = "contentObjectValue")]
pub fn content_object_value(value_json: String) -> Result<Option<String>> {
    Ok(from_json::<Content>(&value_json)?
        .as_object()
        .map(ref_string))
}

#[napi(js_name = "contentIdValue")]
pub fn content_id_value(value_json: String) -> Result<String> {
    Ok(from_json::<ContentId>(&value_json)?.as_str().to_owned())
}

#[napi(js_name = "matrixTranspose")]
pub fn matrix_transpose(value_json: String) -> Result<String> {
    let value: Matrix3 = from_json(&value_json)?;
    to_json(&value.transpose())
}

#[napi(js_name = "matrixBasicRotationId")]
pub fn matrix_basic_rotation_id(value_json: String) -> Result<Option<u8>> {
    Ok(from_json::<Matrix3>(&value_json)?.to_basic_rotation_id())
}

#[napi(js_name = "matrixFromBasicRotationId")]
pub fn matrix_from_basic_rotation_id(id: u32) -> Result<String> {
    to_json(
        &Matrix3::from_basic_rotation_id(checked_u8(id, "id")?)
            .map_err(|error| upstream_error("Matrix3::from_basic_rotation_id", error))?,
    )
}

#[napi(js_name = "materialColorsGet")]
pub fn material_colors_get(value_json: String, material: String) -> Result<String> {
    let value: MaterialColors = from_json(&value_json)?;
    let material = material
        .parse::<TerrainMaterials>()
        .map_err(|error| invalid_arg(format!("invalid terrain material: {error}")))?;
    to_json(&value.get_color(material))
}

#[napi(js_name = "materialColorsSet")]
pub fn material_colors_set(
    value_json: String,
    material: String,
    color_json: String,
) -> Result<String> {
    let mut value: MaterialColors = from_json(&value_json)?;
    let material = material
        .parse::<TerrainMaterials>()
        .map_err(|error| invalid_arg(format!("invalid terrain material: {error}")))?;
    let color: Color3uint8 = from_json(&color_json)?;
    value.set_color(material, color);
    to_json(&value)
}
