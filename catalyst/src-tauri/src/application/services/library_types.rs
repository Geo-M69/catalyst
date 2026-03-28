use crate::FeatureResponse;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameStoreMetadataResponse {
    pub developers: Option<Vec<String>>,
    pub publishers: Option<Vec<String>>,
    pub franchise: Option<String>,
    pub release_date: Option<String>,
    pub short_description: Option<String>,
    pub header_image: Option<String>,
    pub has_achievements: Option<bool>,
    pub achievements_count: Option<i64>,
    pub has_cloud_saves: Option<bool>,
    pub cloud_details: Option<String>,
    pub controller_support: Option<String>,
    pub features: Option<Vec<FeatureResponse>>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameFriendActivityEntryResponse {
    pub steam_id: String,
    pub persona_name: String,
    pub avatar_url: Option<String>,
    pub profile_url: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameFriendsActivityResponse {
    pub provider: String,
    pub external_id: String,
    pub played_friends: Vec<GameFriendActivityEntryResponse>,
    pub owned_friends: Vec<GameFriendActivityEntryResponse>,
    pub friend_list_visibility: String,
    pub warning: Option<String>,
    pub last_synced_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameActivityTimelineItemResponse {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub url: Option<String>,
    pub source_label: Option<String>,
    pub presentation: Option<String>,
    pub occurred_at: String,
    pub is_major_update: Option<bool>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameActivityTimelineResponse {
    pub provider: String,
    pub external_id: String,
    pub items: Vec<GameActivityTimelineItemResponse>,
    pub warning: Option<String>,
    pub last_synced_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameAchievementEntryResponse {
    pub api_name: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub unlocked: bool,
    pub unlocked_at: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameAchievementsResponse {
    pub provider: String,
    pub external_id: String,
    pub total: i64,
    pub unlocked_count: i64,
    pub percent: Option<f64>,
    pub entries: Vec<GameAchievementEntryResponse>,
    pub warning: Option<String>,
    pub last_synced_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameTradingCardEntryResponse {
    pub id: String,
    pub name: String,
    pub image_url: Option<String>,
    pub owned_count: i64,
    pub is_owned: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameTradingCardsResponse {
    pub provider: String,
    pub external_id: String,
    pub supported: bool,
    pub badge_level: Option<i64>,
    pub badge_xp: Option<i64>,
    pub total_cards: i64,
    pub owned_cards: i64,
    pub cards: Vec<GameTradingCardEntryResponse>,
    pub warning: Option<String>,
    pub view_url: String,
    pub last_synced_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameDlcEntryResponse {
    pub id: String,
    pub provider: String,
    pub external_id: String,
    pub name: String,
    pub installed: bool,
    pub in_library: bool,
    pub store_url: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameDlcResponse {
    pub provider: String,
    pub external_id: String,
    pub entries: Vec<GameDlcEntryResponse>,
    pub warning: Option<String>,
    pub last_synced_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameReviewEntryResponse {
    pub id: String,
    pub recommended: bool,
    pub text: String,
    pub playtime_minutes: i64,
    pub created_at: String,
    pub likes: i64,
    pub comments: i64,
    pub source: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameReviewResponse {
    pub provider: String,
    pub external_id: String,
    pub review: Option<GameReviewEntryResponse>,
    pub warning: Option<String>,
    pub last_synced_at: String,
}
