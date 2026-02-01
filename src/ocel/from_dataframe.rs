//! Convert Polars DataFrames to OCEL structure

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use polars::prelude::*;
use process_mining::{
    OCEL,
    core::event_data::object_centric::{
        OCELAttributeValue, OCELEvent, OCELEventAttribute, OCELObject, OCELObjectAttribute,
        OCELRelationship, OCELType, OCELTypeAttribute,
    },
};

use crate::ocel::{OCEL2DataFrames, OCEL2DataFramesRef};

use super::{
    OCEL_EVENT_ID_KEY, OCEL_EVENT_TIMESTAMP_KEY, OCEL_EVENT_TYPE_KEY,
    OCEL_OBJECT_ID_KEY, OCEL_OBJECT_ID_2_KEY, OCEL_OBJECT_TYPE_KEY,
    OCEL_QUALIFIER_KEY,
};

/// Convert a Polars DataType to OCEL type string
fn dtype_to_ocel_type(dtype: &DataType) -> String {
    match dtype {
        DataType::Boolean => "boolean".to_string(),
        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 |
        DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 => "integer".to_string(),
        DataType::Float32 | DataType::Float64 => "float".to_string(),
        DataType::Datetime(_, _) | DataType::Date | DataType::Time => "time".to_string(),
        DataType::String | DataType::Categorical(_, _) | DataType::Enum(_, _) => "string".to_string(),
        DataType::Null => "string".to_string(), // Default null columns to string
        _ => "string".to_string(), // Fallback for other types
    }
}

/// Convert a Polars AnyValue to OCELAttributeValue
fn any_value_to_ocel_attr(val: AnyValue) -> OCELAttributeValue {
    match val {
        AnyValue::Null => OCELAttributeValue::Null,
        AnyValue::Boolean(b) => OCELAttributeValue::Boolean(b),
        AnyValue::Int8(i) => OCELAttributeValue::Integer(i as i64),
        AnyValue::Int16(i) => OCELAttributeValue::Integer(i as i64),
        AnyValue::Int32(i) => OCELAttributeValue::Integer(i as i64),
        AnyValue::Int64(i) => OCELAttributeValue::Integer(i),
        AnyValue::UInt8(i) => OCELAttributeValue::Integer(i as i64),
        AnyValue::UInt16(i) => OCELAttributeValue::Integer(i as i64),
        AnyValue::UInt32(i) => OCELAttributeValue::Integer(i as i64),
        AnyValue::UInt64(i) => OCELAttributeValue::Integer(i as i64),
        AnyValue::Float32(f) => OCELAttributeValue::Float(f as f64),
        AnyValue::Float64(f) => OCELAttributeValue::Float(f),
        AnyValue::String(s) => OCELAttributeValue::String(s.to_string()),
        AnyValue::StringOwned(s) => OCELAttributeValue::String(s.to_string()),
        AnyValue::Datetime(nanos, TimeUnit::Nanoseconds, _) => {
            OCELAttributeValue::Time(Utc.timestamp_nanos(nanos).fixed_offset())
        }
        AnyValue::Datetime(micros, TimeUnit::Microseconds, _) => {
            OCELAttributeValue::Time(Utc.timestamp_nanos(micros * 1000).fixed_offset())
        }
        AnyValue::Datetime(millis, TimeUnit::Milliseconds, _) => {
            OCELAttributeValue::Time(Utc.timestamp_millis_opt(millis).unwrap().fixed_offset())
        }
        _ => OCELAttributeValue::String(format!("{:?}", val)),
    }
}

/// Extract string from AnyValue
fn get_string(val: AnyValue) -> String {
    match val {
        AnyValue::String(s) => s.to_string(),
        AnyValue::StringOwned(s) => s.to_string(),
        _ => format!("{:?}", val),
    }
}

/// Extract timestamp from AnyValue as DateTime<FixedOffset>
fn get_timestamp(val: AnyValue) -> DateTime<FixedOffset> {
    match val {
        AnyValue::Datetime(nanos, TimeUnit::Nanoseconds, _) => Utc.timestamp_nanos(nanos).fixed_offset(),
        AnyValue::Datetime(micros, TimeUnit::Microseconds, _) => Utc.timestamp_nanos(micros * 1000).fixed_offset(),
        AnyValue::Datetime(millis, TimeUnit::Milliseconds, _) => {
            Utc.timestamp_millis_opt(millis).unwrap().fixed_offset()
        }
        _ => DateTime::UNIX_EPOCH.fixed_offset(),
    }
}

/// Convert DataFrames to OCEL structure
/// 
/// 
/// # Returns
/// OCEL structure
pub fn df_to_ocel(
    dfs: OCEL2DataFramesRef
) -> Result<OCEL, PolarsError> {
    // Build event type set and object type set
    let mut event_types: HashMap<String, OCELType> = HashMap::new();
    let mut object_types: HashMap<String, OCELType> = HashMap::new();
    
    // Get attribute columns (non-standard columns)
    let event_attr_cols: Vec<String> = dfs.events
        .get_column_names()
        .into_iter()
        .filter(|c| {
            let name = c.as_str();
            name != OCEL_EVENT_ID_KEY && name != OCEL_EVENT_TYPE_KEY && name != OCEL_EVENT_TIMESTAMP_KEY
        })
        .map(|c| c.to_string())
        .collect();
    
    let object_attr_cols: Vec<String> = dfs.objects
        .get_column_names()
        .into_iter()
        .filter(|c| {
            let name = c.as_str();
            name != OCEL_OBJECT_ID_KEY && name != OCEL_OBJECT_TYPE_KEY
        })
        .map(|c| c.to_string())
        .collect();
    
    // Build column type lookups for proper OCEL type inference
    let event_col_types: HashMap<String, String> = event_attr_cols
        .iter()
        .filter_map(|col| {
            dfs.events.column(col.as_str()).ok().map(|c| {
                (col.clone(), dtype_to_ocel_type(c.dtype()))
            })
        })
        .collect();
    
    let object_col_types: HashMap<String, String> = object_attr_cols
        .iter()
        .filter_map(|col| {
            dfs.objects.column(col.as_str()).ok().map(|c| {
                (col.clone(), dtype_to_ocel_type(c.dtype()))
            })
        })
        .collect();
    
    // Build E2O lookup: event_id -> [(object_id, qualifier)]
    let mut e2o_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for i in 0..dfs.e2o.height() {
        let eid = get_string(dfs.e2o.column(OCEL_EVENT_ID_KEY)?.get(i)?);
        let oid = get_string(dfs.e2o.column(OCEL_OBJECT_ID_KEY)?.get(i)?);
        let qual = get_string(dfs.e2o.column(OCEL_QUALIFIER_KEY)?.get(i)?);
        e2o_map.entry(eid).or_default().push((oid, qual));
    }
    
    // Build O2O lookup: object_id -> [(target_object_id, qualifier)]
    let mut o2o_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for i in 0..dfs.o2o.height() {
        let oid = get_string(dfs.o2o.column(OCEL_OBJECT_ID_KEY)?.get(i)?);
        let oid2 = get_string(dfs.o2o.column(OCEL_OBJECT_ID_2_KEY)?.get(i)?);
        let qual = get_string(dfs.o2o.column(OCEL_QUALIFIER_KEY)?.get(i)?);
        o2o_map.entry(oid).or_default().push((oid2, qual));
    }
    
    // Build events
    let mut events: Vec<OCELEvent> = Vec::with_capacity(dfs.events.height());
    for i in 0..dfs.events.height() {
        let eid = get_string(dfs.events.column(OCEL_EVENT_ID_KEY)?.get(i)?);
        let activity = get_string(dfs.events.column(OCEL_EVENT_TYPE_KEY)?.get(i)?);
        let timestamp = get_timestamp(dfs.events.column(OCEL_EVENT_TIMESTAMP_KEY)?.get(i)?);
        
        // Collect attributes
        let mut attributes: Vec<OCELEventAttribute> = Vec::new();
        for col in &event_attr_cols {
            let val = dfs.events.column(col.as_str())?.get(i)?;
            if !val.is_null() {
                attributes.push(OCELEventAttribute {
                    name: col.clone(),
                    value: any_value_to_ocel_attr(val),
                });
            }
        }
        
        // Get related objects as OCELRelationship
        let relationships: Vec<OCELRelationship> = e2o_map
            .get(&eid)
            .map(|rels| {
                rels.iter()
                    .map(|(oid, qual)| OCELRelationship {
                        object_id: oid.clone(),
                        qualifier: qual.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        
        // Track event type
        event_types.entry(activity.clone()).or_insert_with(|| OCELType {
            name: activity.clone(),
            attributes: event_attr_cols.iter().map(|c| OCELTypeAttribute {
                name: c.clone(),
                value_type: event_col_types.get(c).cloned().unwrap_or_else(|| "string".to_string()),
            }).collect(),
        });
        
        events.push(OCELEvent {
            id: eid,
            event_type: activity,
            time: timestamp,
            attributes,
            relationships,
        });
    }
    
    // Build objects
    let mut objects: Vec<OCELObject> = Vec::with_capacity(dfs.objects.height());
    for i in 0..dfs.objects.height() {
        let oid = get_string(dfs.objects.column(OCEL_OBJECT_ID_KEY)?.get(i)?);
        let obj_type = get_string(dfs.objects.column(OCEL_OBJECT_TYPE_KEY)?.get(i)?);
        
        // Collect attributes (as initial attributes at UNIX_EPOCH)
        let mut attributes: Vec<OCELObjectAttribute> = Vec::new();
        for col in &object_attr_cols {
            let val = dfs.objects.column(col.as_str())?.get(i)?;
            if !val.is_null() {
                attributes.push(OCELObjectAttribute {
                    name: col.clone(),
                    value: any_value_to_ocel_attr(val),
                    time: DateTime::UNIX_EPOCH.fixed_offset(),
                });
            }
        }
        
        // Get O2O relationships
        let relationships: Vec<OCELRelationship> = o2o_map
            .get(&oid)
            .map(|rels| {
                rels.iter()
                    .map(|(oid2, qual)| OCELRelationship {
                        object_id: oid2.clone(),
                        qualifier: qual.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        
        // Track object type
        object_types.entry(obj_type.clone()).or_insert_with(|| OCELType {
            name: obj_type.clone(),
            attributes: object_attr_cols.iter().map(|c| OCELTypeAttribute {
                name: c.clone(),
                value_type: object_col_types.get(c).cloned().unwrap_or_else(|| "string".to_string()),
            }).collect(),
        });
        
        objects.push(OCELObject {
            id: oid,
            object_type: obj_type,
            attributes,
            relationships,
        });
    }
    
    Ok(OCEL {
        event_types: event_types.into_values().collect(),
        object_types: object_types.into_values().collect(),
        events,
        objects,
    })
}
