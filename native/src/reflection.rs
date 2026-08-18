#![allow(dead_code)]

use napi::bindgen_prelude::Buffer;
use napi::Result;
use napi_derive::napi;
use rbx_reflection::ReflectionDatabase as UpstreamReflectionDatabase;
use rbx_reflection_database::get_bundled;
use serde::Serialize;
use serde_json::Value;

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
