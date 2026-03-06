import { invoke } from "@tauri-apps/api/core";
import { IpcError, normalizeAppError } from "./errors";
import type {
  AddGameToCollectionRequest,
  CreateCollectionRequest,
  DeleteCollectionRequest,
  InstallGameRequest,
  IpcCommandName,
  IpcContracts,
  ListCollectionsForGameRequest,
  ProviderExternalIdRequest,
  RenameCollectionRequest,
  SetGameFavoriteRequest,
  SetGamePrivacySettingsRequest,
  SetGamePropertiesSettingsRequest,
  ValidateGameBetaAccessCodeRequest,
} from "./contracts";

type RequestFor<K extends IpcCommandName> = IpcContracts[K]["req"];
type ResponseFor<K extends IpcCommandName> = IpcContracts[K]["res"];

export const callCommand = async <K extends IpcCommandName>(
  command: K,
  payload?: RequestFor<K>
): Promise<ResponseFor<K>> => {
  try {
    if (payload === undefined) {
      return await invoke<ResponseFor<K>>(command);
    }
    return await invoke<ResponseFor<K>>(command, payload as unknown as Record<string, unknown>);
  } catch (error) {
    throw new IpcError(
      normalizeAppError(error, `Command '${command}' failed`),
      { cause: error }
    );
  }
};

export const ipcService = {
  getSession: () => callCommand("get_session"),
  startSteamAuth: () => callCommand("start_steam_auth"),
  logout: () => callCommand("logout"),
  syncSteamLibrary: () => callCommand("sync_steam_library"),
  importSteamCollections: () => callCommand("import_steam_collections"),
  getLibrary: () => callCommand("get_library"),
  listCollections: (payload?: ListCollectionsForGameRequest) => callCommand("list_collections", payload),
  createCollection: (payload: CreateCollectionRequest) => callCommand("create_collection", payload),
  renameCollection: (payload: RenameCollectionRequest) => callCommand("rename_collection", payload),
  deleteCollection: (payload: DeleteCollectionRequest) => callCommand("delete_collection", payload),
  addGameToCollection: (payload: AddGameToCollectionRequest) => callCommand("add_game_to_collection", payload),
  listGameLanguages: (payload: ProviderExternalIdRequest) => callCommand("list_game_languages", payload),
  listGameCompatibilityTools: (payload: ProviderExternalIdRequest) =>
    callCommand("list_game_compatibility_tools", payload),
  listGameVersionBetas: (payload: ProviderExternalIdRequest) => callCommand("list_game_versions_betas", payload),
  validateGameBetaAccessCode: (payload: ValidateGameBetaAccessCodeRequest) =>
    callCommand("validate_game_beta_access_code", payload),
  getGamePrivacySettings: (payload: ProviderExternalIdRequest) => callCommand("get_game_privacy_settings", payload),
  setGamePrivacySettings: (payload: SetGamePrivacySettingsRequest) => callCommand("set_game_privacy_settings", payload),
  clearGameOverlayData: (payload: ProviderExternalIdRequest) => callCommand("clear_game_overlay_data", payload),
  getGameInstallationDetails: (payload: ProviderExternalIdRequest) =>
    callCommand("get_game_installation_details", payload),
  getGameCustomizationArtwork: (payload: ProviderExternalIdRequest) =>
    callCommand("get_game_customization_artwork", payload),
  listGameInstallLocations: (payload: ProviderExternalIdRequest) => callCommand("list_game_install_locations", payload),
  getGameInstallSizeEstimate: (payload: ProviderExternalIdRequest) =>
    callCommand("get_game_install_size_estimate", payload),
  listSteamDownloads: () => callCommand("list_steam_downloads"),
  getGamePropertiesSettings: (payload: ProviderExternalIdRequest) =>
    callCommand("get_game_properties_settings", payload),
  setGamePropertiesSettings: (payload: SetGamePropertiesSettingsRequest) =>
    callCommand("set_game_properties_settings", payload),
  browseGameInstalledFiles: (payload: ProviderExternalIdRequest) =>
    callCommand("browse_game_installed_files", payload),
  backupGameFiles: (payload: ProviderExternalIdRequest) => callCommand("backup_game_files", payload),
  verifyGameFiles: (payload: ProviderExternalIdRequest) => callCommand("verify_game_files", payload),
  addGameDesktopShortcut: (payload: ProviderExternalIdRequest) =>
    callCommand("add_game_desktop_shortcut", payload),
  openGameRecordingSettings: (payload: ProviderExternalIdRequest) =>
    callCommand("open_game_recording_settings", payload),
  uninstallGame: (payload: ProviderExternalIdRequest) => callCommand("uninstall_game", payload),
  installGame: (payload: InstallGameRequest) => callCommand("install_game", payload),
  playGame: (payload: ProviderExternalIdRequest) => callCommand("play_game", payload),
  setGameFavorite: (payload: SetGameFavoriteRequest) => callCommand("set_game_favorite", payload),
};
