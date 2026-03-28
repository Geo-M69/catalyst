export interface GameVersionBetaOption {
  id: string;
  name: string;
  description: string;
  lastUpdated: string;
  buildId?: string;
  requiresAccessCode?: boolean;
  isDefault?: boolean;
}

export interface GameCompatibilityToolOption {
  id: string;
  label: string;
}

export interface GameBetaAccessCodeValidationResult {
  valid: boolean;
  message: string;
  branchId?: string;
  branchName?: string;
}

export interface GameInstallationDetails {
  installPath?: string;
  sizeOnDiskBytes?: number;
}

export interface GameCustomizationArtworkPaths {
  cover?: string;
  background?: string;
  logo?: string;
  wideCover?: string;
}

export interface GameGeneralSettings {
  language: string;
  launchOptions: string;
  steamOverlayEnabled: boolean;
}

export interface GameCompatibilitySettings {
  forceSteamPlayCompatibilityTool: boolean;
  steamPlayCompatibilityTool: string;
}

export type AutomaticUpdatesMode =
  | "use-global-setting"
  | "wait-until-launch"
  | "let-steam-decide"
  | "immediately-download";

export type BackgroundDownloadsMode =
  | "pause-while-playing-global"
  | "always-allow"
  | "never-allow";

export interface GameUpdatesSettings {
  automaticUpdatesMode: AutomaticUpdatesMode;
  backgroundDownloadsMode: BackgroundDownloadsMode;
}

export type SteamInputOverrideMode =
  | "use-default-settings"
  | "disable-steam-input"
  | "enable-steam-input";

export interface GameControllerSettings {
  steamInputOverride: SteamInputOverrideMode;
}

export interface GamePrivacySettings {
  hideInLibrary: boolean;
  markAsPrivate: boolean;
  overlayDataDeleted: boolean;
}

export type GameVersionBetaId = string;

export interface GameVersionsBetasSettings {
  privateAccessCode: string;
  selectedVersionId: GameVersionBetaId;
}

export interface GameCustomizationSettings {
  customSortName: string;
}

export interface GamePropertiesPersistedSettings {
  compatibility: GameCompatibilitySettings;
  customization: GameCustomizationSettings;
  controller: GameControllerSettings;
  gameVersionsBetas: GameVersionsBetasSettings;
  general: GameGeneralSettings;
  updates: GameUpdatesSettings;
}
