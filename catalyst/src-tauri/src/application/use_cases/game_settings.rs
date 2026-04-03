use crate::application::contracts::game_settings::{
    GameCompatibilityToolResponse, GameCustomizationArtworkResponse, GameInstallLocationResponse,
    GameInstallationDetailsResponse, GamePrivacySettingsResponse, GamePropertiesSettingsPayload,
    GameScreenshotResponse,
};
use crate::application::error::AppResult;

pub(crate) trait GameSettingsUseCase {
    fn list_game_languages(&self, provider: String, external_id: String) -> AppResult<Vec<String>>;

    fn list_game_compatibility_tools(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Vec<GameCompatibilityToolResponse>>;

    fn get_game_privacy_settings(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GamePrivacySettingsResponse>;

    fn set_game_privacy_settings(
        &self,
        provider: String,
        external_id: String,
        hide_in_library: bool,
        mark_as_private: bool,
    ) -> AppResult<()>;

    fn clear_game_overlay_data(&self, provider: String, external_id: String) -> AppResult<()>;

    fn get_game_properties_settings(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GamePropertiesSettingsPayload>;

    fn set_game_properties_settings(
        &self,
        provider: String,
        external_id: String,
        settings: GamePropertiesSettingsPayload,
    ) -> AppResult<()>;

    fn get_game_customization_artwork(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GameCustomizationArtworkResponse>;

    fn get_game_screenshots(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Vec<GameScreenshotResponse>>;

    fn get_game_installation_details(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GameInstallationDetailsResponse>;

    fn get_game_install_size_estimate(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Option<u64>>;

    fn list_game_install_locations(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Vec<GameInstallLocationResponse>>;
}
