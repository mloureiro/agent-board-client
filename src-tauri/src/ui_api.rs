use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiApi {
    pub ui_api_version: u32,
    pub min_ui_server: u32,
    pub events_feature: String,
    pub handoff_feature: String,
}

impl UiApi {
    pub fn bundled() -> &'static UiApi {
        static BUNDLED: LazyLock<UiApi> = LazyLock::new(|| {
            serde_json::from_str(include_str!("../../fixtures/ui-api.json"))
                .expect("fixtures/ui-api.json holds the UI API numbers")
        });
        &BUNDLED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_bundled_numbers() {
        let api = UiApi::bundled();

        assert!(api.ui_api_version >= api.min_ui_server);
        assert!(!api.events_feature.is_empty());
        assert!(!api.handoff_feature.is_empty());
    }
}
