import type {
  GameBetaAccessCodeValidationResult,
  GameCompatibilityToolOption,
  GamePropertiesPersistedSettings,
  GameVersionBetaOption,
} from "../../mainPage/components/gamePropertiesPanel";
import type { CollectionResponse, LibraryResponse, PublicUser } from "../../mainPage/types";

export type AppErrorKind = "validation" | "unauthorized" | "not_found" | "conflict" | "external" | "internal";

export interface AppErrorPayload {
  kind: AppErrorKind;
  code: string;
  message: string;
}

export interface SteamAuthResponse {
  user: PublicUser;
  syncedGames: number;
}

export interface GamePrivacySettingsPayload {
  hideInLibrary: boolean;
  markAsPrivate: boolean;
  overlayDataDeleted: boolean;
}

export interface GameInstallationDetailsPayload {
  installPath?: string;
  sizeOnDiskBytes?: number;
}

export interface GameCustomizationArtworkPayload {
  cover?: string;
  background?: string;
  logo?: string;
  wideCover?: string;
}

export interface GameScreenshotPayload {
  id: string;
  path: string;
  thumbnailPath?: string | null;
}

export interface GameStoreMetadataPayload {
  developers?: string[];
  publishers?: string[];
  franchise?: string | null;
  releaseDate?: string | null;
  shortDescription?: string | null;
  headerImage?: string | null;
  hasAchievements?: boolean;
  achievementsCount?: number | null;
  hasCloudSaves?: boolean;
  cloudDetails?: string | null;
  controllerSupport?: string | null;
  features?: FeaturePayload[];
}

export interface FeaturePayload {
  key: string;
  label: string;
  icon?: string | null;
  tooltip?: string | null;
}

export interface GameInstallLocationPayload {
  path: string;
  freeSpaceBytes?: number;
}

export interface SteamDownloadProgressPayload {
  gameId: string;
  provider: string;
  externalId: string;
  name: string;
  state: string;
  bytesDownloaded?: number;
  bytesTotal?: number;
  progressPercent?: number;
  progressSource?: string;
}

export interface GameFriendActivityEntryPayload {
  steamId: string;
  personaName: string;
  avatarUrl?: string;
  profileUrl?: string;
}

export interface GameFriendsActivityPayload {
  provider: string;
  externalId: string;
  playedFriends: GameFriendActivityEntryPayload[];
  ownedFriends: GameFriendActivityEntryPayload[];
  friendListVisibility: "public" | "private" | "unknown";
  warning?: string;
  lastSyncedAt: string;
}

export type GameActivityTimelineItemKind = "achievement" | "news";
export type GameActivityTimelineNewsPresentation = "compact" | "featured";

export interface GameActivityTimelineItemPayload {
  id: string;
  kind: GameActivityTimelineItemKind;
  title: string;
  subtitle?: string;
  description?: string;
  imageUrl?: string;
  url?: string;
  sourceLabel?: string;
  presentation?: GameActivityTimelineNewsPresentation;
  occurredAt: string;
  isMajorUpdate?: boolean;
}

export interface GameActivityTimelinePayload {
  provider: string;
  externalId: string;
  items: GameActivityTimelineItemPayload[];
  warning?: string;
  lastSyncedAt: string;
}

export interface GameAchievementEntryPayload {
  apiName: string;
  name: string;
  description?: string;
  icon?: string | null;
  unlocked: boolean;
  unlockedAt?: string | null;
}

export interface GameAchievementsPayload {
  provider: string;
  externalId: string;
  total: number;
  unlockedCount: number;
  percent?: number | null;
  entries: GameAchievementEntryPayload[];
  warning?: string;
  lastSyncedAt: string;
}

export interface GameTradingCardsPayload {
  provider: string;
  externalId: string;
  supported: boolean;
  badgeLevel?: number | null;
  badgeXp?: number | null;
  totalCards: number;
  ownedCards: number;
  cards: GameTradingCardEntryPayload[];
  warning?: string;
  viewUrl: string;
  lastSyncedAt: string;
}

export interface GameTradingCardEntryPayload {
  id: string;
  name: string;
  imageUrl?: string | null;
  ownedCount: number;
  isOwned: boolean;
}

export interface GameDlcEntryPayload {
  id: string;
  provider: string;
  externalId: string;
  name: string;
  installed: boolean;
  inLibrary: boolean;
  storeUrl: string;
}

export interface GameDlcPayload {
  provider: string;
  externalId: string;
  entries: GameDlcEntryPayload[];
  warning?: string;
  lastSyncedAt: string;
}

export interface GameReviewEntryPayload {
  id: string;
  recommended: boolean;
  text: string;
  playtimeMinutes: number;
  createdAt: string;
  likes: number;
  comments: number;
  source: string;
}

export interface GameReviewPayload {
  provider: string;
  externalId: string;
  review?: GameReviewEntryPayload | null;
  warning?: string;
  lastSyncedAt: string;
}

export interface GameVersionBetasPayload {
  options: GameVersionBetaOption[];
  warning?: string;
}

export interface ProviderExternalIdRequest {
  provider: string;
  externalId: string;
}

export interface SetGamePrivacySettingsRequest extends ProviderExternalIdRequest {
  hideInLibrary: boolean;
  markAsPrivate: boolean;
}

export interface SetGamePropertiesSettingsRequest extends ProviderExternalIdRequest {
  settings: GamePropertiesPersistedSettings;
}

export interface AddGameToCollectionRequest extends ProviderExternalIdRequest {
  collectionId: string;
}

export interface InstallGameRequest extends ProviderExternalIdRequest {
  installPath: string;
  createDesktopShortcut: boolean;
  createApplicationShortcut: boolean;
}

export interface SetGameFavoriteRequest extends ProviderExternalIdRequest {
  favorite: boolean;
}

export interface GetGameFriendsActivityRequest extends ProviderExternalIdRequest {
  forceRefresh?: boolean;
}

export interface GetGameActivityTimelineRequest extends ProviderExternalIdRequest {
  forceRefresh?: boolean;
}

export interface GetGameTradingCardsRequest extends ProviderExternalIdRequest {
  forceRefresh?: boolean;
}

export interface GetGameDlcRequest extends ProviderExternalIdRequest {
  forceRefresh?: boolean;
}

export interface GetGameReviewRequest extends ProviderExternalIdRequest {
  forceRefresh?: boolean;
}

export interface ValidateGameBetaAccessCodeRequest extends ProviderExternalIdRequest {
  accessCode: string;
}

export interface ListCollectionsForGameRequest extends ProviderExternalIdRequest {}

export interface RenameCollectionRequest {
  collectionId: string;
  name: string;
}

export interface DeleteCollectionRequest {
  collectionId: string;
}

export interface CreateCollectionRequest {
  name: string;
}

export interface IpcContracts {
  get_session: { req: void; res: PublicUser | null };
  start_steam_auth: { req: void; res: SteamAuthResponse };
  logout: { req: void; res: void };
  start_local_steam_scan: { req: void; res: void };
  sync_steam_library: { req: void; res: void };
  import_steam_collections: { req: void; res: void };
  get_library: { req: void; res: LibraryResponse };
  list_collections: { req: void | ListCollectionsForGameRequest; res: CollectionResponse[] };
  create_collection: { req: CreateCollectionRequest; res: CollectionResponse };
  rename_collection: { req: RenameCollectionRequest; res: CollectionResponse };
  delete_collection: { req: DeleteCollectionRequest; res: void };
  add_game_to_collection: { req: AddGameToCollectionRequest; res: void };
  list_game_languages: { req: ProviderExternalIdRequest; res: string[] };
  list_game_compatibility_tools: { req: ProviderExternalIdRequest; res: GameCompatibilityToolOption[] };
  list_game_versions_betas: { req: ProviderExternalIdRequest; res: GameVersionBetasPayload };
  validate_game_beta_access_code: {
    req: ValidateGameBetaAccessCodeRequest;
    res: GameBetaAccessCodeValidationResult;
  };
  get_game_privacy_settings: { req: ProviderExternalIdRequest; res: GamePrivacySettingsPayload };
  set_game_privacy_settings: { req: SetGamePrivacySettingsRequest; res: void };
  clear_game_overlay_data: { req: ProviderExternalIdRequest; res: void };
  get_game_installation_details: { req: ProviderExternalIdRequest; res: GameInstallationDetailsPayload };
  get_game_customization_artwork: { req: ProviderExternalIdRequest; res: GameCustomizationArtworkPayload };
  get_game_store_metadata: { req: ProviderExternalIdRequest; res: GameStoreMetadataPayload };
  list_game_install_locations: { req: ProviderExternalIdRequest; res: GameInstallLocationPayload[] };
  get_game_install_size_estimate: { req: ProviderExternalIdRequest; res: number | null };
  list_steam_downloads: { req: void; res: SteamDownloadProgressPayload[] };
  get_game_friends_activity: { req: GetGameFriendsActivityRequest; res: GameFriendsActivityPayload };
  get_game_activity_timeline: { req: GetGameActivityTimelineRequest; res: GameActivityTimelinePayload };
  get_game_achievements: { req: GetGameActivityTimelineRequest; res: GameAchievementsPayload };
  get_game_trading_cards: { req: GetGameTradingCardsRequest; res: GameTradingCardsPayload };
  get_game_dlc: { req: GetGameDlcRequest; res: GameDlcPayload };
  get_game_review: { req: GetGameReviewRequest; res: GameReviewPayload };
  get_game_properties_settings: { req: ProviderExternalIdRequest; res: GamePropertiesPersistedSettings };
  set_game_properties_settings: { req: SetGamePropertiesSettingsRequest; res: void };
  browse_game_installed_files: { req: ProviderExternalIdRequest; res: void };
  backup_game_files: { req: ProviderExternalIdRequest; res: void };
  verify_game_files: { req: ProviderExternalIdRequest; res: void };
  get_game_screenshots: { req: ProviderExternalIdRequest; res: GameScreenshotPayload[] };
  add_game_desktop_shortcut: { req: ProviderExternalIdRequest; res: void };
  open_game_recording_settings: { req: ProviderExternalIdRequest; res: void };
  uninstall_game: { req: ProviderExternalIdRequest; res: void };
  install_game: { req: InstallGameRequest; res: void };
  play_game: { req: ProviderExternalIdRequest; res: void };
  set_game_favorite: { req: SetGameFavoriteRequest; res: void };
}

export type IpcCommandName = keyof IpcContracts;
