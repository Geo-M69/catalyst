#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameCompatibilityToolResponse {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GamePrivacySettingsResponse {
    pub hide_in_library: bool,
    pub mark_as_private: bool,
    pub overlay_data_deleted: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameInstallationDetailsResponse {
    pub install_path: Option<String>,
    pub size_on_disk_bytes: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameInstallLocationResponse {
    pub path: String,
    pub free_space_bytes: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameGeneralSettingsPayload {
    pub language: String,
    pub launch_options: String,
    pub steam_overlay_enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameCompatibilitySettingsPayload {
    pub force_steam_play_compatibility_tool: bool,
    pub steam_play_compatibility_tool: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameUpdatesSettingsPayload {
    pub automatic_updates_mode: String,
    pub background_downloads_mode: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameControllerSettingsPayload {
    pub steam_input_override: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameVersionsBetasSettingsPayload {
    pub private_access_code: String,
    pub selected_version_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameCustomizationSettingsPayload {
    pub custom_sort_name: String,
}

fn default_game_customization_settings_payload() -> GameCustomizationSettingsPayload {
    GameCustomizationSettingsPayload {
        custom_sort_name: String::new(),
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GamePropertiesSettingsPayload {
    pub general: GameGeneralSettingsPayload,
    pub compatibility: GameCompatibilitySettingsPayload,
    pub updates: GameUpdatesSettingsPayload,
    pub controller: GameControllerSettingsPayload,
    #[serde(default = "default_game_customization_settings_payload")]
    pub customization: GameCustomizationSettingsPayload,
    pub game_versions_betas: GameVersionsBetasSettingsPayload,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameCustomizationArtworkResponse {
    pub cover: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub wide_cover: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameScreenshotResponse {
    pub id: String,
    pub path: String,
    pub thumbnail_path: Option<String>,
}
