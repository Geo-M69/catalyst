#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameVersionBetaOptionResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub last_updated: String,
    pub build_id: Option<String>,
    pub requires_access_code: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameVersionBetasResponse {
    pub options: Vec<GameVersionBetaOptionResponse>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameBetaAccessCodeValidationResponse {
    pub valid: bool,
    pub message: String,
    pub branch_id: Option<String>,
    pub branch_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SteamCollectionsImportResponse {
    pub apps_tagged: usize,
    pub collections_created: usize,
    pub memberships_added: usize,
    pub skipped_games: usize,
    pub tags_discovered: usize,
}
