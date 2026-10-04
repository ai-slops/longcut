use longcut_session_contract::model::Package;

pub fn fixture() -> Result<Package, serde_json::Error> {
    Ok(Package {
        manifest: serde_json::from_str(include_str!(
            "../../../../fixtures/edit-session-packages/complete/manifest.json"
        ))?,
        registry: serde_json::from_str(include_str!(
            "../../../../fixtures/edit-session-packages/complete/sources.json"
        ))?,
        time_maps: Some(serde_json::from_str(include_str!(
            "../../../../fixtures/edit-session-packages/complete/time-maps.json"
        ))?),
        visual_composition: Some(serde_json::from_str(include_str!(
            "../../../../fixtures/edit-session-packages/complete/visual-composition.json"
        ))?),
        audio_mix: Some(serde_json::from_str(include_str!(
            "../../../../fixtures/edit-session-packages/complete/audio-mix.json"
        ))?),
        media_health: Some(serde_json::from_str(include_str!(
            "../../../../fixtures/edit-session-packages/complete/media-health.json"
        ))?),
    })
}
