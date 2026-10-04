mod media;
mod time;
mod tracks;

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use media::*;
pub use time::*;
pub use tracks::*;

pub type Extensions = BTreeMap<String, serde_json::Value>;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub contract: Contract,
    #[schemars(regex(pattern = r"^0\.1\.(0|[1-9][0-9]*)$"))]
    pub version: String,
    pub package_state: PackageState,
    pub session_id: String,
    pub duration: Duration,
    pub sources_file: String,
    pub program_reference_source_id: Option<SourceId>,
    pub time_maps_file: Option<String>,
    pub visual_composition_file: Option<String>,
    pub audio_mix_file: Option<String>,
    pub media_health_file: Option<String>,
    #[serde(default)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub enum Contract {
    #[serde(rename = "edit-session-package")]
    EditSessionPackage,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PackageState {
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Package {
    pub manifest: Manifest,
    pub registry: Registry,
    pub time_maps: Option<TimeMaps>,
    pub visual_composition: Option<VisualComposition>,
    pub audio_mix: Option<AudioMix>,
    pub media_health: Option<MediaHealth>,
}
