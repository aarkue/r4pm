use std::collections::{HashMap, HashSet};

use polars::{prelude::*, series::Series};
use process_mining::{
    OCEL,
    core::event_data::object_centric::OCELAttributeValue,
};
use pyo3_polars::PyDataFrame;

use crate::ocel::{OCEL_CHANGED_FIELD_KEY, OCEL_EVENT_ID_KEY, OCEL_EVENT_TIMESTAMP_KEY, OCEL_EVENT_TYPE_KEY, OCEL_OBJECT_ID_2_KEY, OCEL_OBJECT_ID_KEY, OCEL_OBJECT_TYPE_KEY, OCEL_QUALIFIER_KEY, OCEL2DataFrames};
fn ocel_attribute_val_to_any_value<'a>(
    val: &'a OCELAttributeValue,
) -> AnyValue<'a> {
    match val {
        OCELAttributeValue::String(s) => AnyValue::StringOwned(s.into()),
        OCELAttributeValue::Time(t) => AnyValue::Datetime(
            t.timestamp_nanos_opt().unwrap(),
            TimeUnit::Nanoseconds,
            None,
        ),
        OCELAttributeValue::Integer(i) => AnyValue::Int64(*i),
        OCELAttributeValue::Float(f) => AnyValue::Float64(*f),
        OCELAttributeValue::Boolean(b) => AnyValue::Boolean(*b),
        OCELAttributeValue::Null => AnyValue::Null,
    }
}

pub fn ocel2_to_df(ocel: &OCEL) -> OCEL2DataFrames {
    let object_attributes: HashSet<String> = ocel
        .object_types
        .iter()
        .flat_map(|ot| &ot.attributes)
        .map(|at| at.name.clone())
        .collect();
    let actual_object_attributes: HashSet<String> = ocel
        .objects
        .iter()
        .flat_map(|o| o.attributes.iter().map(|oa| oa.name.clone()))
        .collect();
    if !object_attributes.is_superset(&actual_object_attributes) {
        eprintln!(
            "Warning: Global object attributes is not a superset of actual object attributes"
        );
    }
    // PM4Py uses the first attributes value (index 0) in the objects DF (i.e., each object attribute is always a column in objects)
    // We match this behavior.
    let object_attributes_initial: &HashSet<String> = &object_attributes;
        // .clone()
        // .into_iter()
        // .filter(|a| {
        //     ocel.objects.iter().any(|o| {
        //         o.attributes
        //             .iter()
        //             .any(|oa| &oa.name == a && oa.time == DateTime::UNIX_EPOCH)
        //     })
        // })
        // .collect();
    let objects_df = DataFrame::from_iter(
        object_attributes_initial
            .into_iter()
            .map(|name| {
                Series::from_any_values(
                    (name).into(),
                    ocel.objects
                        .iter()
                        .map(|o| {
                            let attr = o
                                .attributes
                                .iter()
                                .find(|a| &a.name == name);
                            let val = match attr {
                                Some(v) => &v.value,
                                None => &OCELAttributeValue::Null,
                            };
                            ocel_attribute_val_to_any_value(val)
                        })
                        .collect::<Vec<_>>()
                        .as_ref(),
                    false,
                )
                .unwrap()
            })
            .chain(vec![
                Series::from_any_values(
                    OCEL_OBJECT_ID_KEY.into(),
                    &ocel
                        .objects
                        .iter()
                        .map(|o| AnyValue::StringOwned(o.id.clone().into()))
                        .collect::<Vec<_>>(),
                    false,
                )
                .unwrap(),
                Series::from_any_values(
                    OCEL_OBJECT_TYPE_KEY.into(),
                    &ocel
                        .objects
                        .iter()
                        .map(|o| AnyValue::StringOwned(o.object_type.clone().into()))
                        .collect::<Vec<_>>(),
                    false,
                )
                .unwrap(),
            ]),
    );

    let all_evs_with_rels: Vec<_> = ocel
        .events
        .iter()
        .flat_map(|e| {
            e.relationships
                .iter()
                .map(move |r| (e, r))
        })
        .collect();

    let obj_id_to_type_map: HashMap<&String, &String> = ocel
        .objects
        .iter()
        .map(|o| (&o.id, &o.object_type))
        .collect();

    let mut e2o_df = DataFrame::from_iter(vec![
        Series::from_any_values(
            OCEL_EVENT_ID_KEY.into(),
            &all_evs_with_rels
                .iter()
                .map(|(e, _r)| AnyValue::StringOwned(e.id.clone().into()))
                .collect::<Vec<_>>(),
            false,
        )
        .unwrap(),
        Series::from_any_values(
            OCEL_EVENT_TYPE_KEY.into(),
            &all_evs_with_rels
                .iter()
                .map(|(e, _r)| AnyValue::StringOwned(e.event_type.clone().into()))
                .collect::<Vec<_>>(),
            false,
        )
        .unwrap(),
        Series::from_any_values(
            OCEL_EVENT_TIMESTAMP_KEY.into(),
            &all_evs_with_rels
                .iter()
                .map(|(e, _r)| {
                    AnyValue::Datetime(
                        e.time.timestamp_nanos_opt().unwrap(),
                        TimeUnit::Nanoseconds,
                        None,
                    )
                })
                .collect::<Vec<_>>(),
            false,
        )
        .unwrap(),
        Series::from_any_values(
            OCEL_OBJECT_ID_KEY.into(),
            &all_evs_with_rels
                .iter()
                .map(|(_e, r)| AnyValue::StringOwned(r.object_id.clone().into()))
                .collect::<Vec<_>>(),
            false,
        )
        .unwrap(),
        Series::from_any_values(
            OCEL_OBJECT_TYPE_KEY.into(),
            &all_evs_with_rels
                .iter()
                .map(|(_e, r)| {
                    if let Some(obj_type) = obj_id_to_type_map.get(&r.object_id) {
                        AnyValue::StringOwned((*obj_type).into())
                    } else {
                        // eprintln!(
                        //     "Invalid object id in E2O reference: Event: {}, Object: {}",
                        //     _e.id, r.object_id
                        // );
                        AnyValue::Null
                    }
                })
                .collect::<Vec<_>>(),
            false,
        )
        .unwrap(),
        Series::from_any_values(
            OCEL_QUALIFIER_KEY.into(),
            &all_evs_with_rels
                .iter()
                .map(|(_e, r)| AnyValue::StringOwned(r.qualifier.clone().into()))
                .collect::<Vec<_>>(),
            false,
        )
        .unwrap(),
    ]);

    let all_obj_with_rels: Vec<_> = ocel
        .objects
        .iter()
        .flat_map(|o| {
            o.relationships
                .iter()
                .map(move |r| (o, r))
        })
        .collect();

    let o2o_df = DataFrame::from_iter(vec![
        Series::from_any_values(
            OCEL_OBJECT_ID_KEY.into(),
            &all_obj_with_rels
                .iter()
                .map(|(o, _r)| AnyValue::StringOwned(o.id.clone().into()))
                .collect::<Vec<_>>(),
            false,
        )
        .unwrap(),
        Series::from_any_values(
            OCEL_OBJECT_ID_2_KEY.into(),
            &all_obj_with_rels
                .iter()
                .map(|(_o, r)| AnyValue::StringOwned(r.object_id.clone().into()))
                .collect::<Vec<_>>(),
            false,
        )
        .unwrap(),
        Series::from_any_values(
            OCEL_QUALIFIER_KEY.into(),
            &all_obj_with_rels
                .iter()
                .map(|(_o, r)| AnyValue::StringOwned(r.qualifier.clone().into()))
                .collect::<Vec<_>>(),
            false,
        )
        .unwrap(),
    ]);

    // The first value of each object attribute is already exported in the objects DF (see above),
    // so only the later values are changes. This matches PM4Py, whose object_changes DF also
    // excludes the initial value.
    let object_attribute_changes: Vec<_> = ocel
        .objects
        .iter()
        .flat_map(|o| {
            let mut seen: HashSet<&str> = HashSet::new();
            o.attributes
                .iter()
                .filter(move |a| !seen.insert(a.name.as_str()))
                .map(move |a| (o, a))
        })
        .collect();

    let mut object_changes_df = DataFrame::from_iter(
        object_attributes
            .into_iter()
            .map(|name| {
                Series::from_any_values(
                    (&name).into(),
                    object_attribute_changes
                        .iter()
                        .map(|(_o, a)| {
                            if a.name == name {
                                ocel_attribute_val_to_any_value(&a.value)
                            } else {
                                AnyValue::Null
                            }
                        })
                        .collect::<Vec<_>>()
                        .as_ref(),
                    false,
                )
                .unwrap()
            })
            .chain(vec![
                Series::from_any_values(
                    OCEL_OBJECT_ID_KEY.into(),
                    &object_attribute_changes
                        .iter()
                        .map(|(o, _a)| AnyValue::StringOwned(o.id.clone().into()))
                        .collect::<Vec<_>>(),
                    false,
                )
                .unwrap(),
                Series::from_any_values(
                    OCEL_OBJECT_TYPE_KEY.into(),
                    &object_attribute_changes
                        .iter()
                        .map(|(o, _a)| AnyValue::StringOwned(o.object_type.clone().into()))
                        .collect::<Vec<_>>(),
                    false,
                )
                .unwrap(),
                Series::from_any_values(
                    OCEL_CHANGED_FIELD_KEY.into(),
                    &object_attribute_changes
                        .iter()
                        .map(|(_o, a)| AnyValue::StringOwned(a.name.clone().into()))
                        .collect::<Vec<_>>(),
                    false,
                )
                .unwrap(),
                Series::from_any_values(
                    OCEL_EVENT_TIMESTAMP_KEY.into(),
                    &object_attribute_changes
                        .iter()
                        .map(|(_o, a)| {
                            AnyValue::Datetime(
                                a.time.timestamp_nanos_opt().unwrap(),
                                TimeUnit::Nanoseconds,
                                None,
                            )
                        })
                        .collect::<Vec<_>>(),
                    false,
                )
                .unwrap(),
            ]),
    );
    let event_attributes: HashSet<String> = ocel
        .event_types
        .iter()
        .flat_map(|et| &et.attributes)
        .map(|at| at.name.clone())
        .collect();
    let mut events_df = DataFrame::from_iter(
        event_attributes
            .into_iter()
            .map(|name| {
                Series::from_any_values(
                    (&name).into(),
                    ocel.events
                        .iter()
                        .map(|e| {
                            let attr = e.attributes.iter().find(|a| a.name == name);
                            let val = match attr {
                                Some(v) => &v.value,
                                None => &OCELAttributeValue::Null,
                            };
                            ocel_attribute_val_to_any_value(val)
                        })
                        .collect::<Vec<_>>()
                        .as_ref(),
                    false,
                )
                .unwrap()
            })
            .chain(vec![
                Series::from_any_values(
                    OCEL_EVENT_ID_KEY.into(),
                    &ocel
                        .events
                        .iter()
                        .map(|o| AnyValue::StringOwned(o.id.clone().into()))
                        .collect::<Vec<_>>(),
                    false,
                )
                .unwrap(),
                Series::from_any_values(
                    OCEL_EVENT_TYPE_KEY.into(),
                    &ocel
                        .events
                        .iter()
                        .map(|o| AnyValue::StringOwned(o.event_type.clone().into()))
                        .collect::<Vec<_>>(),
                    false,
                )
                .unwrap(),
                Series::from_any_values(
                    OCEL_EVENT_TIMESTAMP_KEY.into(),
                    &ocel
                        .events
                        .iter()
                        .map(|o| {
                            AnyValue::Datetime(
                                o.time.timestamp_nanos_opt().unwrap(),
                                TimeUnit::Nanoseconds,
                                None,
                            )
                        })
                        .collect::<Vec<_>>(),
                    false,
                )
                .unwrap(),
            ]),
    );
    events_df
        .sort_in_place(vec![OCEL_EVENT_TIMESTAMP_KEY], SortMultipleOptions::default().with_maintain_order(true))
        .unwrap();

    e2o_df
        .sort_in_place(vec![OCEL_EVENT_TIMESTAMP_KEY], SortMultipleOptions::default().with_maintain_order(true))
        .unwrap();

    object_changes_df
        .sort_in_place(vec![OCEL_EVENT_TIMESTAMP_KEY], SortMultipleOptions::default().with_maintain_order(true))
        .unwrap();
    OCEL2DataFrames {
        objects: objects_df,
        events: events_df,
        object_changes: object_changes_df,
        o2o: o2o_df,
        e2o: e2o_df,
    }
}

pub fn ocel_dfs_to_py(ocel_dfs: OCEL2DataFrames) -> HashMap<String, PyDataFrame> {
    let mut res: HashMap<String, PyDataFrame> = HashMap::with_capacity(5);
    res.insert("events".to_string(), PyDataFrame(ocel_dfs.events));
    res.insert("objects".to_string(), PyDataFrame(ocel_dfs.objects));
    res.insert("o2o".to_string(), PyDataFrame(ocel_dfs.o2o));
    res.insert("relations".to_string(), PyDataFrame(ocel_dfs.e2o));
    res.insert(
        "object_changes".to_string(),
        PyDataFrame(ocel_dfs.object_changes),
    );
    res
}