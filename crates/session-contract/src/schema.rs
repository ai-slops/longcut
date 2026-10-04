use std::collections::BTreeMap;

use schemars::{Schema, schema_for};

use crate::model::*;

pub fn schemas() -> BTreeMap<&'static str, Schema> {
    BTreeMap::from([
        ("manifest", schema_for!(Manifest)),
        ("sources", schema_for!(Registry)),
        ("time-maps", schema_for!(TimeMaps)),
        ("visual-composition", schema_for!(VisualComposition)),
        ("audio-mix", schema_for!(AudioMix)),
        ("media-health", schema_for!(MediaHealth)),
    ])
}
