#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicUser {
    pub id: String,
    pub email: String,
    pub steam_linked: bool,
    pub steam_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SteamAuthResponse {
    pub user: PublicUser,
    pub synced_games: usize,
}
