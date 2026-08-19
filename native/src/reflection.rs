#![allow(dead_code)]

use napi::bindgen_prelude::Buffer;
use napi::Result;
use napi_derive::napi;
use rbx_reflection::ReflectionDatabase as UpstreamReflectionDatabase;
use rbx_reflection_database::get_bundled;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::invalid_arg;

pub(crate) fn normalize_database_value(value: &Value) -> Value {
    fn normalize(value: Value, in_array: bool) -> Value {
        match value {
            Value::Null if in_array => Value::from(0.0),
            Value::Object(mut object) => {
                for (key, value) in &mut object {
                    if key == "Float32" && value.is_null() {
                        *value = Value::from(0.0);
                    } else {
                        *value = normalize(value.take(), false);
                    }
                }
                Value::Object(object)
            }
            Value::Array(values) => Value::Array(
                values
                    .into_iter()
                    .map(|value| normalize(value, true))
                    .collect(),
            ),
            value => value,
        }
    }

    normalize(value.clone(), false)
}

pub(crate) fn normalize_database_json(value: &Value) -> Result<String> {
    serde_json::to_string(&normalize_database_value(value))
        .map_err(|error| crate::error::upstream_error("serializing reflection database", error))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BindingMetadata {
    upstream_commit: &'static str,
    crates: Vec<CrateMetadata>,
    packages: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CrateMetadata {
    name: &'static str,
    version: &'static str,
    kind: &'static str,
}

fn to_json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value)
        .map_err(|error| crate::error::upstream_error("serializing reflection value", error))
}

fn required_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_arg(format!("API dump field {key:?} must be a string")))
}

fn known_tags(value: Option<&Value>, known: &[&str]) -> Value {
    Value::Array(
        value
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|tag| known.contains(tag))
            .map(|tag| Value::String(tag.to_owned()))
            .collect(),
    )
}

fn variant_type_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "Axes" => "Axes",
        "BinaryString" => "BinaryString",
        "bool" => "Bool",
        "BrickColor" => "BrickColor",
        "CFrame" => "CFrame",
        "Color3" => "Color3",
        "Color3uint8" => "Color3uint8",
        "ColorSequence" => "ColorSequence",
        "Content" => "Content",
        "ContentId" => "ContentId",
        "Faces" => "Faces",
        "Font" => "Font",
        "Instance" => "Ref",
        "NetAssetRef" => "NetAssetRef",
        "NumberRange" => "NumberRange",
        "NumberSequence" => "NumberSequence",
        "OptionalCoordinateFrame" => "OptionalCFrame",
        "PhysicalProperties" => "PhysicalProperties",
        "Ray" => "Ray",
        "Rect" => "Rect",
        "Region3" => "Region3",
        "Region3int16" => "Region3int16",
        "SecurityCapabilities" => "SecurityCapabilities",
        "SharedString" => "SharedString",
        "UDim" => "UDim",
        "UDim2" => "UDim2",
        "UniqueId" => "UniqueId",
        "Vector2" => "Vector2",
        "Vector2int16" => "Vector2int16",
        "Vector3" => "Vector3",
        "Vector3int16" => "Vector3int16",
        "double" => "Float64",
        "float" => "Float32",
        "int" => "Int32",
        "int64" => "Int64",
        "string" | "ProtectedString" => "String",
        _ => return None,
    })
}

fn scriptability(property: &Map<String, Value>, tags: &[&str]) -> &'static str {
    if tags.contains(&"NotScriptable") {
        return "None";
    }
    let security = property
        .get("Security")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let read = matches!(
        security.get("Read").and_then(Value::as_str),
        Some("None" | "PluginSecurity")
    );
    let write = if tags.contains(&"ReadOnly") {
        false
    } else {
        matches!(
            security.get("Write").and_then(Value::as_str),
            Some("None" | "PluginSecurity")
        )
    };
    match (read, write) {
        (true, true) => "ReadWrite",
        (true, false) => "Read",
        (false, true) => "Write",
        (false, false) => "None",
    }
}

fn reflection_database_from_api_dump_json(api_dump_json: &str) -> Result<String> {
    let dump: Value = serde_json::from_str(api_dump_json)
        .map_err(|error| invalid_arg(format!("invalid Roblox API dump JSON: {error}")))?;
    let dump = dump
        .as_object()
        .ok_or_else(|| invalid_arg("Roblox API dump must be an object"))?;
    let classes = dump
        .get("Classes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_arg("Roblox API dump is missing a Classes array"))?;
    let enums = dump
        .get("Enums")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_arg("Roblox API dump is missing an Enums array"))?;

    let class_tags = [
        "Deprecated",
        "NotBrowsable",
        "NotCreatable",
        "NotReplicated",
        "PlayerReplicated",
        "Service",
        "Settings",
        "UserSettings",
    ];
    let property_tags = [
        "Deprecated",
        "Hidden",
        "NotBrowsable",
        "NotReplicated",
        "NotScriptable",
        "ReadOnly",
        "WriteOnly",
    ];
    let mut output_classes = Map::new();
    for class in classes {
        let class = class
            .as_object()
            .ok_or_else(|| invalid_arg("API dump class must be an object"))?;
        let name = required_string(class, "Name")?;
        let superclass = required_string(class, "Superclass")?;
        let mut properties = Map::new();
        if let Some(members) = class.get("Members").and_then(Value::as_array) {
            for member in members {
                let member = member
                    .as_object()
                    .ok_or_else(|| invalid_arg("API dump member must be an object"))?;
                if member.get("MemberType").and_then(Value::as_str) != Some("Property") {
                    continue;
                }
                let property_name = required_string(member, "Name")?;
                let value_type = member
                    .get("ValueType")
                    .and_then(Value::as_object)
                    .ok_or_else(|| invalid_arg("API dump property is missing ValueType"))?;
                let type_name = required_string(value_type, "Name")?;
                let category = required_string(value_type, "Category")?;
                let data_type = match category {
                    "Enum" => serde_json::json!({ "Enum": type_name }),
                    "Class" => serde_json::json!({ "Value": "Ref" }),
                    "Primitive" | "DataType" => match variant_type_name(type_name) {
                        Some(type_name) => serde_json::json!({ "Value": type_name }),
                        None => continue,
                    },
                    _ => continue,
                };
                let tags = known_tags(member.get("Tags"), &property_tags);
                let tag_names: Vec<_> = tags
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                let can_save = member
                    .get("Serialization")
                    .and_then(Value::as_object)
                    .and_then(|serialization| serialization.get("CanSave"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                properties.insert(
                    property_name.to_owned(),
                    serde_json::json!({
                        "Name": property_name,
                        "Scriptability": scriptability(member, &tag_names),
                        "DataType": data_type,
                        "Tags": tags,
                        "Kind": { "Canonical": { "Serialization": if can_save {
                            "Serializes"
                        } else {
                            "DoesNotSerialize"
                        } } }
                    }),
                );
            }
        }
        let mut class_value = serde_json::json!({
            "Name": name,
            "Tags": known_tags(class.get("Tags"), &class_tags),
            "Properties": properties,
            "DefaultProperties": {},
        });
        if superclass != "<<<ROOT>>>" {
            class_value["Superclass"] = Value::String(superclass.to_owned());
        }
        output_classes.insert(name.to_owned(), class_value);
    }

    let mut output_enums = Map::new();
    for descriptor in enums {
        let descriptor = descriptor
            .as_object()
            .ok_or_else(|| invalid_arg("API dump enum must be an object"))?;
        let name = required_string(descriptor, "Name")?;
        let mut items = Map::new();
        if let Some(values) = descriptor.get("Items").and_then(Value::as_array) {
            for item in values {
                let item = item
                    .as_object()
                    .ok_or_else(|| invalid_arg("API dump enum item must be an object"))?;
                let item_name = required_string(item, "Name")?;
                let item_value = item
                    .get("Value")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid_arg("API dump enum item Value must be an integer"))?;
                items.insert(item_name.to_owned(), Value::from(item_value));
            }
        }
        output_enums.insert(
            name.to_owned(),
            serde_json::json!({ "name": name, "items": items }),
        );
    }

    serde_json::to_string(&serde_json::json!({
        "Version": [0, 0, 0, 0],
        "Classes": output_classes,
        "Enums": output_enums,
    }))
    .map_err(|error| crate::error::upstream_error("serializing reflection database", error))
}

#[napi]
pub struct ReflectionDatabase {
    json: Option<String>,
    bytes: Option<Vec<u8>>,
}

#[napi]
impl ReflectionDatabase {
    #[napi(constructor)]
    pub fn new(database_json: Option<String>) -> Result<Self> {
        let json = database_json
            .map(|json| {
                let value: Value = serde_json::from_str(&json).map_err(|error| {
                    invalid_arg(format!("invalid reflection database JSON: {error}"))
                })?;
                let json = normalize_database_json(&value)?;
                serde_json::from_str::<UpstreamReflectionDatabase<'_>>(&json).map_err(|error| {
                    invalid_arg(format!("invalid reflection database JSON: {error}"))
                })?;
                Ok::<_, napi::Error>(json)
            })
            .transpose()?;
        Ok(Self { json, bytes: None })
    }

    #[napi(factory, js_name = "fromApiDump")]
    pub fn from_api_dump(api_dump_json: String) -> Result<Self> {
        let json = reflection_database_from_api_dump_json(&api_dump_json)?;
        serde_json::from_str::<UpstreamReflectionDatabase<'_>>(&json).map_err(|error| {
            invalid_arg(format!("generated reflection database is invalid: {error}"))
        })?;
        Ok(Self {
            json: Some(json),
            bytes: None,
        })
    }

    pub(crate) fn parsed(&self) -> Result<UpstreamReflectionDatabase<'_>> {
        match &self.bytes {
            Some(bytes) => rmp_serde::from_slice(bytes)
                .map_err(|error| invalid_arg(format!("invalid reflection database: {error}"))),
            None => match &self.json {
                Some(json) => serde_json::from_str(json).map_err(|error| {
                    invalid_arg(format!("invalid reflection database JSON: {error}"))
                }),
                None => Ok(get_bundled().clone()),
            },
        }
    }

    #[napi(js_name = "version")]
    pub fn version(&self) -> Result<Vec<u32>> {
        Ok(self.parsed()?.version.to_vec())
    }

    #[napi(js_name = "classNames")]
    pub fn class_names(&self) -> Result<Vec<String>> {
        let database = self.parsed()?;
        let mut names: Vec<_> = database
            .classes
            .keys()
            .map(|name| (*name).to_owned())
            .collect();
        names.sort_unstable();
        Ok(names)
    }

    #[napi(js_name = "enumNames")]
    pub fn enum_names(&self) -> Result<Vec<String>> {
        let database = self.parsed()?;
        let mut names: Vec<_> = database
            .enums
            .keys()
            .map(|name| (*name).to_owned())
            .collect();
        names.sort_unstable();
        Ok(names)
    }

    #[napi(js_name = "toJson")]
    pub fn to_json(&self) -> String {
        self.json.clone().unwrap_or_else(|| {
            self.bytes
                .as_ref()
                .and_then(|bytes| {
                    rmp_serde::from_slice::<UpstreamReflectionDatabase<'_>>(bytes)
                        .ok()
                        .and_then(|database| to_json(&database).ok())
                })
                .unwrap_or_else(|| to_json(get_bundled()).unwrap_or_else(|_| "{}".to_owned()))
        })
    }

    #[napi(js_name = "class")]
    pub fn class(&self, name: String) -> Result<String> {
        let database = self.parsed()?;
        to_json(&database.classes.get(name.as_str()))
    }

    #[napi(js_name = "property")]
    pub fn property(&self, class_name: String, property_name: String) -> Result<String> {
        let database = self.parsed()?;
        to_json(
            &database
                .classes
                .get(class_name.as_str())
                .and_then(|class| class.properties.get(property_name.as_str())),
        )
    }

    #[napi(js_name = "defaultProperty")]
    pub fn default_property(&self, class_name: String, property_name: String) -> Result<String> {
        let database = self.parsed()?;
        let value = database
            .classes
            .get(class_name.as_str())
            .and_then(|class| database.find_default_property(class, &property_name));
        to_json(&value)
    }

    #[napi(js_name = "propertyNames")]
    pub fn property_names(&self, class_name: String) -> Result<Vec<String>> {
        let database = self.parsed()?;
        let class = database
            .classes
            .get(class_name.as_str())
            .ok_or_else(|| invalid_arg(format!("unknown reflection class {class_name:?}")))?;
        let mut names: Vec<_> = class
            .properties
            .keys()
            .map(|name| (*name).to_owned())
            .collect();
        names.sort_unstable();
        Ok(names)
    }

    #[napi(js_name = "enum")]
    pub fn enum_descriptor(&self, name: String) -> Result<String> {
        to_json(&self.parsed()?.enums.get(name.as_str()))
    }

    #[napi(js_name = "enumItems")]
    pub fn enum_items(&self, name: String) -> Result<String> {
        let database = self.parsed()?;
        let items = database.enums.get(name.as_str()).map(|descriptor| {
            descriptor
                .items
                .iter()
                .map(|(item, value)| ((*item).to_owned(), *value))
                .collect::<std::collections::BTreeMap<_, _>>()
        });
        to_json(&items)
    }

    #[napi(js_name = "isA")]
    pub fn is_a(&self, class_name: String, superclass_name: String) -> Result<bool> {
        let database = self.parsed()?;
        let Some(class) = database.classes.get(class_name.as_str()) else {
            return Ok(false);
        };
        let Some(superclass) = database.classes.get(superclass_name.as_str()) else {
            return Ok(false);
        };
        Ok(database.has_superclass(class, superclass))
    }

    #[napi(js_name = "superclasses")]
    pub fn superclasses(&self, class_name: String) -> Result<Vec<String>> {
        let database = self.parsed()?;
        let class = database
            .classes
            .get(class_name.as_str())
            .ok_or_else(|| invalid_arg(format!("unknown reflection class {class_name:?}")))?;
        Ok(database
            .superclasses_iter(class)
            .map(|descriptor| descriptor.name.to_owned())
            .collect())
    }
}

#[napi(js_name = "reflectionDatabaseFromBinary")]
pub fn reflection_database_from_binary(data: Buffer) -> Result<ReflectionDatabase> {
    rmp_serde::from_slice::<UpstreamReflectionDatabase<'_>>(&data)
        .map_err(|error| invalid_arg(format!("invalid reflection database: {error}")))?;
    Ok(ReflectionDatabase {
        json: None,
        bytes: Some(data.to_vec()),
    })
}

#[napi(js_name = "reflectionVersion")]
pub fn reflection_version() -> Vec<u32> {
    get_bundled().version.to_vec()
}

#[napi(js_name = "reflectionClassNames")]
pub fn reflection_class_names() -> Vec<String> {
    let mut names: Vec<_> = get_bundled()
        .classes
        .keys()
        .map(|name| (*name).to_owned())
        .collect();
    names.sort_unstable();
    names
}

#[napi(js_name = "reflectionEnumNames")]
pub fn reflection_enum_names() -> Vec<String> {
    let mut names: Vec<_> = get_bundled()
        .enums
        .keys()
        .map(|name| (*name).to_owned())
        .collect();
    names.sort_unstable();
    names
}

#[napi(js_name = "reflectionDatabase")]
pub fn reflection_database() -> Result<String> {
    to_json(get_bundled())
}

#[napi(js_name = "reflectionClass")]
pub fn reflection_class(name: String) -> Result<String> {
    to_json(&get_bundled().classes.get(name.as_str()))
}

#[napi(js_name = "reflectionProperty")]
pub fn reflection_property(class_name: String, property_name: String) -> Result<String> {
    let descriptor = get_bundled()
        .classes
        .get(class_name.as_str())
        .and_then(|class| class.properties.get(property_name.as_str()));
    to_json(&descriptor)
}

#[napi(js_name = "reflectionDefaultProperty")]
pub fn reflection_default_property(class_name: String, property_name: String) -> Result<String> {
    let value = get_bundled()
        .classes
        .get(class_name.as_str())
        .and_then(|class| get_bundled().find_default_property(class, &property_name));
    to_json(&value)
}

#[napi(js_name = "reflectionPropertyNames")]
pub fn reflection_property_names(class_name: String) -> Result<Vec<String>> {
    let class = get_bundled()
        .classes
        .get(class_name.as_str())
        .ok_or_else(|| invalid_arg(format!("unknown reflection class {class_name:?}")))?;
    let mut names: Vec<_> = class
        .properties
        .keys()
        .map(|name| (*name).to_owned())
        .collect();
    names.sort_unstable();
    Ok(names)
}

#[napi(js_name = "reflectionEnum")]
pub fn reflection_enum(name: String) -> Result<String> {
    to_json(&get_bundled().enums.get(name.as_str()))
}

#[napi(js_name = "reflectionEnumItems")]
pub fn reflection_enum_items(name: String) -> Result<String> {
    let items = get_bundled().enums.get(name.as_str()).map(|descriptor| {
        descriptor
            .items
            .iter()
            .map(|(item, value)| ((*item).to_owned(), *value))
            .collect::<std::collections::BTreeMap<_, _>>()
    });
    to_json(&items)
}

#[napi(js_name = "reflectionIsA")]
pub fn reflection_is_a(class_name: String, superclass_name: String) -> bool {
    let database = get_bundled();
    let Some(class) = database.classes.get(class_name.as_str()) else {
        return false;
    };
    let Some(superclass) = database.classes.get(superclass_name.as_str()) else {
        return false;
    };
    database.has_superclass(class, superclass)
}

#[napi(js_name = "reflectionSuperclasses")]
pub fn reflection_superclasses(class_name: String) -> Result<Vec<String>> {
    let database = get_bundled();
    let class = database
        .classes
        .get(class_name.as_str())
        .ok_or_else(|| invalid_arg(format!("unknown reflection class {class_name:?}")))?;
    Ok(database
        .superclasses_iter(class)
        .map(|descriptor| descriptor.name.to_owned())
        .collect())
}

#[napi(js_name = "reflectionLocalDatabasePath")]
pub fn reflection_local_database_path() -> Option<String> {
    rbx_reflection_database::get_local_location().map(|path| path.to_string_lossy().into_owned())
}

#[napi(js_name = "bindingMetadata")]
pub fn binding_metadata() -> Result<String> {
    let metadata = BindingMetadata {
        upstream_commit: "43d1f129f2eb1fd055512f039863ff35ae5a10f1",
        crates: vec![
            CrateMetadata {
                name: "rbx_types",
                version: "3.1.0",
                kind: "library",
            },
            CrateMetadata {
                name: "rbx_dom_weak",
                version: "4.2.0",
                kind: "library",
            },
            CrateMetadata {
                name: "rbx_reflection",
                version: "7.0.0",
                kind: "library",
            },
            CrateMetadata {
                name: "rbx_reflection_database",
                version: "3.0.0+roblox-728",
                kind: "library",
            },
            CrateMetadata {
                name: "rbx_xml",
                version: "3.0.0",
                kind: "library",
            },
            CrateMetadata {
                name: "rbx_binary",
                version: "3.0.0",
                kind: "library",
            },
            CrateMetadata {
                name: "rbx_reflector",
                version: "0.1.0",
                kind: "upstream-binary",
            },
            CrateMetadata {
                name: "rbx_util",
                version: "0.2.1",
                kind: "upstream-binary",
            },
        ],
        packages: vec!["rbx_dom_lua (Luau package, Roblox runtime only)"],
    };
    to_json(&metadata)
}
