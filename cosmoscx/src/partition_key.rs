// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! FFI functions for computing effective partition key (EPK) strings.
//! These wrap the hashing logic in `azure_data_cosmos_engine::hash` and expose a C ABI.

use azure_data_cosmos_engine::ErrorKind;
use azure_data_cosmos_engine::{
    get_hashed_partition_key_string, PartitionKeyKind, PartitionKeyValue,
};
use serde_json::Value;

use crate::{
    result::{FfiResult, ResultExt},
    slice::{OwnedString, Str},
};

pub const COSMOSCX_PARTITION_KEY_KIND_HASH: u8 = 0;
pub const COSMOSCX_PARTITION_KEY_KIND_MULTI_HASH: u8 = 1;
pub const COSMOSCX_PARTITION_KEY_VERSION_V1: u8 = 1;
pub const COSMOSCX_PARTITION_KEY_VERSION_V2: u8 = 2;

/// Parses a JSON value into a vector of `PartitionKeyValue`s accepted by the hashing function.
fn parse_partition_key_components(
    v: Value,
) -> Result<Vec<PartitionKeyValue>, azure_data_cosmos_engine::Error> {
    fn convert(val: &Value) -> Result<PartitionKeyValue, azure_data_cosmos_engine::Error> {
        Ok(match val {
            Value::Null => PartitionKeyValue::Null,
            Value::Bool(b) => PartitionKeyValue::Bool(*b),
            Value::Number(n) => {
                PartitionKeyValue::Number(n.as_f64().ok_or_else(|| {
                    ErrorKind::DeserializationError.with_message("invalid number")
                })?)
            }
            Value::String(s) => PartitionKeyValue::String(s.clone()),
            Value::Object(map) => {
                // Only the empty JSON object is allowed and means Undefined / Missing PK component
                if map.is_empty() {
                    PartitionKeyValue::Undefined
                } else {
                    return Err(ErrorKind::DeserializationError
                        .with_message("non-empty object not allowed in partition key"));
                }
            }
            Value::Array(_) => {
                return Err(ErrorKind::DeserializationError
                    .with_message("nested arrays not allowed in partition key"));
            }
        })
    }

    match v {
        Value::String(ref s) if s == "Infinity" => {
            // Special case: top-level "Infinity" represents the boundary value
            Ok(vec![PartitionKeyValue::Infinity])
        }
        Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr.iter() {
                out.push(convert(item)?);
            }
            Ok(out)
        }
        // Allow passing a single value instead of an array; wrap it.
        other => Ok(vec![convert(&other)?]),
    }
}

/// Computes an effective partition key string for the provided JSON representation.
///
/// Parameters:
/// - `partition_key_json`: JSON representing either a single value (e.g. `"abc"`) or an array (e.g. `["abc", 5]`).
/// - `version`: 1 for V1, 2 for V2.
/// - `kind`: 0 for Hash, 1 for MultiHash.
///
/// Returns: An engine-owned UTF-8 string (hex) that must be freed with `cosmoscx_v0_partition_key_free_string`.
#[no_mangle]
pub extern "C" fn cosmoscx_v0_partition_key_effective<'a>(
    partition_key_json: Str<'a>,
    version: u8,
    kind: u8,
) -> FfiResult<OwnedString> {
    fn inner_compute<'a>(
        json: Str<'a>,
        version: u8,
        kind: u8,
    ) -> Result<Box<OwnedString>, azure_data_cosmos_engine::Error> {
        let json_str = unsafe { json.as_str().not_null()? };
        // Empty string not allowed (would imply none)
        if json_str.is_empty() {
            return Err(ErrorKind::DeserializationError.with_message("empty partition key json"));
        }
        let value: Value = serde_json::from_str(json_str)
            .map_err(|e| ErrorKind::DeserializationError.with_source(e))?;
        let components = parse_partition_key_components(value)?;

        let pk_kind = match kind {
            COSMOSCX_PARTITION_KEY_KIND_HASH => PartitionKeyKind::Hash,
            COSMOSCX_PARTITION_KEY_KIND_MULTI_HASH => PartitionKeyKind::MultiHash,
            _ => {
                return Err(
                    ErrorKind::DeserializationError.with_message("invalid partition key kind")
                )
            }
        };

        if pk_kind == PartitionKeyKind::MultiHash && version != COSMOSCX_PARTITION_KEY_VERSION_V2 {
            return Err(
                ErrorKind::DeserializationError.with_message("MultiHash only supports version 2")
            );
        }

        let epk = get_hashed_partition_key_string(&components, Some(pk_kind), Some(version));
        Ok(Box::new(epk.into()))
    }
    inner_compute(partition_key_json, version, kind).into()
}
