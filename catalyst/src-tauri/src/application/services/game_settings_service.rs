use crate::application::contracts::game_settings::{
    GameCompatibilityToolResponse,
    GameCustomizationArtworkResponse,
    GameInstallLocationResponse,
    GameInstallationDetailsResponse,
    GamePrivacySettingsResponse,
    GamePropertiesSettingsPayload,
    GameScreenshotResponse,
};
use crate::application::error::AppResult;
use crate::application::ports::game_settings::GameSettingsPort;
use crate::application::use_cases::game_settings::GameSettingsUseCase;

pub(crate) struct GameSettingsService<P> {
    port: P,
}

impl<P> GameSettingsService<P>
where
    P: GameSettingsPort,
{
    pub(crate) fn new(port: P) -> Self {
        Self { port }
    }
}

impl<P> GameSettingsUseCase for GameSettingsService<P>
where
    P: GameSettingsPort,
{
    fn list_game_languages(&self, provider: String, external_id: String) -> AppResult<Vec<String>> {
        self.port.list_game_languages(provider, external_id)
    }

    fn list_game_compatibility_tools(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Vec<GameCompatibilityToolResponse>> {
        self.port
            .list_game_compatibility_tools(provider, external_id)
    }

    fn get_game_privacy_settings(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GamePrivacySettingsResponse> {
        self.port.get_game_privacy_settings(provider, external_id)
    }

    fn set_game_privacy_settings(
        &self,
        provider: String,
        external_id: String,
        hide_in_library: bool,
        mark_as_private: bool,
    ) -> AppResult<()> {
        self.port.set_game_privacy_settings(
            provider,
            external_id,
            hide_in_library,
            mark_as_private,
        )
    }

    fn clear_game_overlay_data(&self, provider: String, external_id: String) -> AppResult<()> {
        self.port.clear_game_overlay_data(provider, external_id)
    }

    fn get_game_properties_settings(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GamePropertiesSettingsPayload> {
        self.port.get_game_properties_settings(provider, external_id)
    }

    fn set_game_properties_settings(
        &self,
        provider: String,
        external_id: String,
        settings: GamePropertiesSettingsPayload,
    ) -> AppResult<()> {
        self.port
            .set_game_properties_settings(provider, external_id, settings)
    }

    fn get_game_customization_artwork(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GameCustomizationArtworkResponse> {
        self.port.get_game_customization_artwork(provider, external_id)
    }

    fn get_game_screenshots(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Vec<GameScreenshotResponse>> {
        self.port.get_game_screenshots(provider, external_id)
    }

    fn get_game_installation_details(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<GameInstallationDetailsResponse> {
        self.port.get_game_installation_details(provider, external_id)
    }

    fn get_game_install_size_estimate(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Option<u64>> {
        self.port.get_game_install_size_estimate(provider, external_id)
    }

    fn list_game_install_locations(
        &self,
        provider: String,
        external_id: String,
    ) -> AppResult<Vec<GameInstallLocationResponse>> {
        self.port.list_game_install_locations(provider, external_id)
    }
}
