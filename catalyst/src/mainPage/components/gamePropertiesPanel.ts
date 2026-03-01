import type { GameResponse } from "../types";

export interface GamePropertiesInput {
  game: GameResponse;
  collections: string[];
  availableLanguages?: string[];
  availableVersionOptions?: GameVersionBetaOption[];
  availableVersionOptionsWarning?: string;
  persistedSettings?: GamePropertiesPersistedSettings;
  saveSettings?: (settings: GamePropertiesPersistedSettings) => Promise<void>;
  installationDetails?: GameInstallationDetails;
  browseInstalledFiles?: () => Promise<void>;
  backupInstalledFiles?: () => Promise<void>;
  verifyInstalledFiles?: () => Promise<void>;
  privacySettings?: GamePrivacySettings;
  setPrivacySettings?: (settings: Pick<GamePrivacySettings, "hideInLibrary" | "markAsPrivate">) => Promise<void>;
  deleteOverlayData?: () => Promise<void>;
  validateBetaAccessCode?: (accessCode: string) => Promise<GameBetaAccessCodeValidationResult>;
}

export interface GamePropertiesPanelController {
  close: () => void;
  open: (input: GamePropertiesInput) => void;
}

export interface GameVersionBetaOption {
  id: string;
  name: string;
  description: string;
  lastUpdated: string;
  buildId?: string;
  requiresAccessCode?: boolean;
  isDefault?: boolean;
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

type GamePropertiesTabId =
  | "general"
  | "compatibility"
  | "updates"
  | "installed-files"
  | "game-versions-betas"
  | "controller"
  | "privacy";

interface GamePropertiesTab {
  id: GamePropertiesTabId;
  label: string;
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

type AutomaticUpdatesMode =
  | "use-global-setting"
  | "wait-until-launch"
  | "let-steam-decide"
  | "immediately-download";

type BackgroundDownloadsMode =
  | "pause-while-playing-global"
  | "always-allow"
  | "never-allow";

export interface GameUpdatesSettings {
  automaticUpdatesMode: AutomaticUpdatesMode;
  backgroundDownloadsMode: BackgroundDownloadsMode;
}

type SteamInputOverrideMode = "use-default-settings" | "disable-steam-input" | "enable-steam-input";

export interface GameControllerSettings {
  steamInputOverride: SteamInputOverrideMode;
}

export interface GamePrivacySettings {
  hideInLibrary: boolean;
  markAsPrivate: boolean;
  overlayDataDeleted: boolean;
}

type GameVersionBetaId = string;

export interface GameVersionsBetasSettings {
  privateAccessCode: string;
  selectedVersionId: GameVersionBetaId;
}

export interface GamePropertiesPersistedSettings {
  compatibility: GameCompatibilitySettings;
  controller: GameControllerSettings;
  gameVersionsBetas: GameVersionsBetasSettings;
  general: GameGeneralSettings;
  updates: GameUpdatesSettings;
}

interface DropdownOption {
  description?: string;
  dividerBefore?: boolean;
  label: string;
  value: string;
}

interface BetaValidationStatus {
  kind: "idle" | "loading" | "error" | "success";
  message: string;
}

interface PrivacyStatus {
  kind: "idle" | "error" | "success";
  message: string;
}

interface InstalledFilesStatus {
  kind: "idle" | "error" | "success";
  message: string;
}

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");
const GAME_PROPERTIES_TABS: readonly GamePropertiesTab[] = [
  { id: "general", label: "General" },
  { id: "compatibility", label: "Compatibility" },
  { id: "updates", label: "Updates" },
  { id: "installed-files", label: "Installed Files" },
  { id: "game-versions-betas", label: "Game Versions & Betas" },
  { id: "controller", label: "Controller" },
  { id: "privacy", label: "Privacy" },
];
const DEFAULT_LANGUAGE_OPTIONS = ["English", "French", "German", "Spanish", "Japanese"];
const STEAM_PLAY_COMPATIBILITY_TOOL_OPTIONS = [
  "Proton Experimental",
  "Proton Hotfix",
  "Proton 9.0-4",
  "Proton 8.0-5",
  "Proton 7.0-6",
  "Steam Linux Runtime 3.0 (sniper)",
  "Steam Linux Runtime 2.0 (soldier)",
];
const DEFAULT_GENERAL_SETTINGS: GameGeneralSettings = {
  language: "English",
  launchOptions: "",
  steamOverlayEnabled: true,
};
const DEFAULT_COMPATIBILITY_SETTINGS: GameCompatibilitySettings = {
  forceSteamPlayCompatibilityTool: false,
  steamPlayCompatibilityTool: STEAM_PLAY_COMPATIBILITY_TOOL_OPTIONS[0],
};
const AUTOMATIC_UPDATES_OPTIONS: readonly DropdownOption[] = [
  {
    value: "use-global-setting",
    label: "Use global setting: Let Steam decide when to update",
    description: "Set from Settings > Downloads",
  },
  {
    value: "wait-until-launch",
    label: "Wait until I launch the game",
    dividerBefore: true,
  },
  {
    value: "let-steam-decide",
    label: "Let Steam decide when to update",
    description: "Considers factors like when you last played the game, etc.",
  },
  {
    value: "immediately-download",
    label: "Immediately download updates",
    description: "Prioritize this game before other downloads",
  },
];
const BACKGROUND_DOWNLOAD_OPTIONS: readonly DropdownOption[] = [
  {
    value: "pause-while-playing-global",
    label: "Pause background downloads while I'm playing",
    description: "(Per my global Steam Settings)",
  },
  {
    value: "always-allow",
    label: "Always allow background downloads",
  },
  {
    value: "never-allow",
    label: "Never allow background downloads",
  },
];
const DEFAULT_UPDATES_SETTINGS: GameUpdatesSettings = {
  automaticUpdatesMode: "use-global-setting",
  backgroundDownloadsMode: "pause-while-playing-global",
};
const CONTROLLER_OVERRIDE_OPTIONS: readonly DropdownOption[] = [
  {
    value: "use-default-settings",
    label: "Use default settings",
  },
  {
    value: "disable-steam-input",
    label: "Disable Steam Input",
  },
  {
    value: "enable-steam-input",
    label: "Enable Steam Input",
  },
];
const CONTROLLER_STATUS_ROWS: ReadonlyArray<{ label: string; status: string }> = [
  { label: "Xbox Controller", status: "" },
  { label: "PlayStation", status: "" },
  { label: "Nintendo Switch", status: "" },
  { label: "Generic Controller", status: "" },
  { label: "Steam Controller", status: "Enabled, always required" },
  { label: "Remote Play", status: "Enabled, always required" },
];
const DEFAULT_CONTROLLER_SETTINGS: GameControllerSettings = {
  steamInputOverride: "use-default-settings",
};
const DEFAULT_PRIVACY_SETTINGS: GamePrivacySettings = {
  hideInLibrary: false,
  markAsPrivate: false,
  overlayDataDeleted: false,
};
const DEFAULT_GAME_VERSION_BETA_OPTIONS: readonly GameVersionBetaOption[] = [
  {
    id: "public",
    isDefault: true,
    requiresAccessCode: false,
    description: "Most common version of the game",
    lastUpdated: "Unavailable",
    name: "Default Public Version",
    buildId: undefined,
  },
] as const;
const DEFAULT_GAME_VERSIONS_BETAS_SETTINGS: GameVersionsBetasSettings = {
  privateAccessCode: "",
  selectedVersionId: "public",
};

const cloneGeneralSettings = (settings: GameGeneralSettings): GameGeneralSettings => {
  return { ...settings };
};

const cloneCompatibilitySettings = (settings: GameCompatibilitySettings): GameCompatibilitySettings => {
  return { ...settings };
};

const cloneUpdatesSettings = (settings: GameUpdatesSettings): GameUpdatesSettings => {
  return { ...settings };
};

const cloneControllerSettings = (settings: GameControllerSettings): GameControllerSettings => {
  return { ...settings };
};

const clonePrivacySettings = (settings: GamePrivacySettings): GamePrivacySettings => {
  return { ...settings };
};

const cloneGameVersionsBetasSettings = (settings: GameVersionsBetasSettings): GameVersionsBetasSettings => {
  return { ...settings };
};

const cloneGameVersionOptions = (options: readonly GameVersionBetaOption[]): GameVersionBetaOption[] => {
  return options.map((option) => ({ ...option }));
};

const isAutomaticUpdatesMode = (value: string): value is AutomaticUpdatesMode => {
  return AUTOMATIC_UPDATES_OPTIONS.some((option) => option.value === value);
};

const isBackgroundDownloadsMode = (value: string): value is BackgroundDownloadsMode => {
  return BACKGROUND_DOWNLOAD_OPTIONS.some((option) => option.value === value);
};

const isSteamInputOverrideMode = (value: string): value is SteamInputOverrideMode => {
  return CONTROLLER_OVERRIDE_OPTIONS.some((option) => option.value === value);
};

const isNonEmptyString = (value: unknown): value is string => {
  return typeof value === "string" && value.trim().length > 0;
};

const toRecord = (value: unknown): Record<string, unknown> | null => {
  if (typeof value !== "object" || value === null) {
    return null;
  }

  return value as Record<string, unknown>;
};

const parseGeneralSettings = (record: Record<string, unknown>): GameGeneralSettings => {
  return {
    steamOverlayEnabled: typeof record.steamOverlayEnabled === "boolean"
      ? record.steamOverlayEnabled
      : DEFAULT_GENERAL_SETTINGS.steamOverlayEnabled,
    language: typeof record.language === "string" && record.language.trim().length > 0
      ? record.language
      : DEFAULT_GENERAL_SETTINGS.language,
    launchOptions: typeof record.launchOptions === "string"
      ? record.launchOptions
      : DEFAULT_GENERAL_SETTINGS.launchOptions,
  };
};

const parseCompatibilitySettings = (record: Record<string, unknown>): GameCompatibilitySettings => {
  return {
    forceSteamPlayCompatibilityTool: typeof record.forceSteamPlayCompatibilityTool === "boolean"
      ? record.forceSteamPlayCompatibilityTool
      : DEFAULT_COMPATIBILITY_SETTINGS.forceSteamPlayCompatibilityTool,
    steamPlayCompatibilityTool: typeof record.steamPlayCompatibilityTool === "string"
      && record.steamPlayCompatibilityTool.trim().length > 0
      ? record.steamPlayCompatibilityTool
      : DEFAULT_COMPATIBILITY_SETTINGS.steamPlayCompatibilityTool,
  };
};

const parseUpdatesSettings = (record: Record<string, unknown>): GameUpdatesSettings => {
  return {
    automaticUpdatesMode: typeof record.automaticUpdatesMode === "string"
      && isAutomaticUpdatesMode(record.automaticUpdatesMode)
      ? record.automaticUpdatesMode
      : DEFAULT_UPDATES_SETTINGS.automaticUpdatesMode,
    backgroundDownloadsMode: typeof record.backgroundDownloadsMode === "string"
      && isBackgroundDownloadsMode(record.backgroundDownloadsMode)
      ? record.backgroundDownloadsMode
      : DEFAULT_UPDATES_SETTINGS.backgroundDownloadsMode,
  };
};

const parseControllerSettings = (record: Record<string, unknown>): GameControllerSettings => {
  return {
    steamInputOverride: typeof record.steamInputOverride === "string"
      && isSteamInputOverrideMode(record.steamInputOverride)
      ? record.steamInputOverride
      : DEFAULT_CONTROLLER_SETTINGS.steamInputOverride,
  };
};

const parseGameVersionsBetasSettings = (record: Record<string, unknown>): GameVersionsBetasSettings => {
  return {
    privateAccessCode: typeof record.privateAccessCode === "string"
      ? record.privateAccessCode
      : DEFAULT_GAME_VERSIONS_BETAS_SETTINGS.privateAccessCode,
    selectedVersionId: isNonEmptyString(record.selectedVersionId)
      ? record.selectedVersionId
      : DEFAULT_GAME_VERSIONS_BETAS_SETTINGS.selectedVersionId,
  };
};

const createDefaultGamePropertiesPersistedSettings = (): GamePropertiesPersistedSettings => {
  return {
    compatibility: cloneCompatibilitySettings(DEFAULT_COMPATIBILITY_SETTINGS),
    controller: cloneControllerSettings(DEFAULT_CONTROLLER_SETTINGS),
    gameVersionsBetas: cloneGameVersionsBetasSettings(DEFAULT_GAME_VERSIONS_BETAS_SETTINGS),
    general: cloneGeneralSettings(DEFAULT_GENERAL_SETTINGS),
    updates: cloneUpdatesSettings(DEFAULT_UPDATES_SETTINGS),
  };
};

const parseGamePropertiesPersistedSettings = (
  input: GamePropertiesPersistedSettings | undefined
): GamePropertiesPersistedSettings => {
  if (!input) {
    return createDefaultGamePropertiesPersistedSettings();
  }

  const inputRecord = toRecord(input) ?? {};
  const generalRecord = toRecord(inputRecord.general) ?? inputRecord;
  const compatibilityRecord = toRecord(inputRecord.compatibility) ?? inputRecord;
  const controllerRecord = toRecord(inputRecord.controller) ?? inputRecord;
  const gameVersionsBetasRecord = toRecord(inputRecord.gameVersionsBetas) ?? inputRecord;
  const updatesRecord = toRecord(inputRecord.updates) ?? inputRecord;

  return {
    compatibility: parseCompatibilitySettings(compatibilityRecord),
    controller: parseControllerSettings(controllerRecord),
    gameVersionsBetas: parseGameVersionsBetasSettings(gameVersionsBetasRecord),
    general: parseGeneralSettings(generalRecord),
    updates: parseUpdatesSettings(updatesRecord),
  };
};

const getFocusableElements = (container: HTMLElement): HTMLElement[] => {
  return Array
    .from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
    .filter((element) => !element.hidden);
};

const normalizeLanguageOptions = (languageOptions: string[]): string[] => {
  const uniqueLanguages = new Set<string>();
  const normalized: string[] = [];

  for (const option of languageOptions) {
    const normalizedOption = option.trim();
    if (normalizedOption.length === 0) {
      continue;
    }

    const dedupeKey = normalizedOption.toLowerCase();
    if (uniqueLanguages.has(dedupeKey)) {
      continue;
    }

    uniqueLanguages.add(dedupeKey);
    normalized.push(normalizedOption);
  }

  return normalized;
};

const normalizeGameVersionBetaOptions = (options: readonly GameVersionBetaOption[]): GameVersionBetaOption[] => {
  const normalized: GameVersionBetaOption[] = [];
  const seen = new Set<string>();

  for (const option of options) {
    const normalizedId = option.id.trim();
    if (normalizedId.length === 0) {
      continue;
    }

    const dedupeKey = normalizedId.toLowerCase();
    if (seen.has(dedupeKey)) {
      continue;
    }

    seen.add(dedupeKey);
    const normalizedName = option.name.trim();
    const normalizedDescription = option.description.trim();
    const normalizedLastUpdated = option.lastUpdated.trim();

    normalized.push({
      id: normalizedId,
      name: normalizedName.length > 0 ? normalizedName : normalizedId,
      description: normalizedDescription.length > 0 ? normalizedDescription : "No description available",
      lastUpdated: normalizedLastUpdated.length > 0 ? normalizedLastUpdated : "Unavailable",
      buildId: option.buildId?.trim() || undefined,
      requiresAccessCode: option.requiresAccessCode === true,
      isDefault: option.isDefault === true,
    });
  }

  return normalized;
};

const resolveLanguageOptions = (
  availableLanguages: string[],
  selectedLanguage: string
): string[] => {
  const resolved = normalizeLanguageOptions(availableLanguages);
  if (resolved.length === 0) {
    resolved.push(...DEFAULT_LANGUAGE_OPTIONS);
  }

  if (!resolved.some((option) => option.toLowerCase() === selectedLanguage.toLowerCase())) {
    resolved.push(selectedLanguage);
  }

  return resolved;
};

const resolveCompatibilityToolOptions = (selectedTool: string): string[] => {
  const resolved = normalizeLanguageOptions(STEAM_PLAY_COMPATIBILITY_TOOL_OPTIONS);
  if (!resolved.some((tool) => tool.toLowerCase() === selectedTool.toLowerCase())) {
    resolved.push(selectedTool);
  }

  return resolved;
};

const resolveGameVersionBetaOptions = (
  availableOptions: readonly GameVersionBetaOption[],
  selectedVersionId: string
): GameVersionBetaOption[] => {
  const resolved = normalizeGameVersionBetaOptions(availableOptions);
  if (resolved.length === 0) {
    resolved.push(...cloneGameVersionOptions(DEFAULT_GAME_VERSION_BETA_OPTIONS));
  }

  const hasSelectedOption = selectedVersionId.trim().length > 0
    && resolved.some((option) => option.id.toLowerCase() === selectedVersionId.toLowerCase());
  if (!hasSelectedOption && selectedVersionId.trim().length > 0) {
    resolved.push({
      id: selectedVersionId,
      name: selectedVersionId,
      description: "Previously selected branch",
      lastUpdated: "Unavailable",
      requiresAccessCode: false,
      isDefault: false,
    });
  }

  const defaultIndex = resolved.findIndex((option) => option.isDefault || option.id.toLowerCase() === "public");
  if (defaultIndex > 0) {
    const [defaultOption] = resolved.splice(defaultIndex, 1);
    resolved.unshift(defaultOption);
  }

  return resolved;
};

const formatTimestampForMetadata = (timestamp: string): string => {
  const parsedTimestamp = new Date(timestamp);
  if (Number.isNaN(parsedTimestamp.getTime())) {
    return "Unavailable";
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(parsedTimestamp);
};

const formatSizeForMetadata = (sizeInBytes: number): string => {
  if (!Number.isFinite(sizeInBytes) || sizeInBytes <= 0) {
    return "Unknown size";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = sizeInBytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const fractionDigits = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(fractionDigits)} ${units[unitIndex]}`;
};

export const createGamePropertiesPanel = (): GamePropertiesPanelController => {
  const backdrop = document.createElement("div");
  backdrop.className = "game-properties-backdrop";
  backdrop.hidden = true;

  const panel = document.createElement("section");
  panel.className = "game-properties-panel";
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-modal", "true");
  panel.setAttribute("aria-labelledby", "game-properties-title");

  const header = document.createElement("header");
  header.className = "game-properties-header";

  const titleBlock = document.createElement("div");
  titleBlock.className = "game-properties-title-block";

  const title = document.createElement("h3");
  title.id = "game-properties-title";
  title.className = "game-properties-title";
  title.textContent = "Properties";

  const subtitle = document.createElement("p");
  subtitle.className = "game-properties-subtitle";

  const closeButton = document.createElement("button");
  closeButton.type = "button";
  closeButton.className = "game-properties-close session-account-item";
  closeButton.textContent = "Close";
  closeButton.setAttribute("aria-label", "Close properties");

  const layout = document.createElement("div");
  layout.className = "game-properties-layout";

  const sidebar = document.createElement("nav");
  sidebar.className = "game-properties-sidebar";
  sidebar.setAttribute("aria-label", "Properties sections");

  const tabList = document.createElement("div");
  tabList.className = "game-properties-tablist";
  tabList.setAttribute("role", "tablist");
  tabList.setAttribute("aria-orientation", "vertical");

  const content = document.createElement("section");
  content.className = "game-properties-content";

  const tabPanel = document.createElement("div");
  tabPanel.id = "game-properties-tab-panel";
  tabPanel.className = "game-properties-tab-panel";
  tabPanel.setAttribute("role", "tabpanel");
  tabPanel.tabIndex = 0;

  sidebar.append(tabList);
  content.append(tabPanel);
  layout.append(sidebar, content);

  titleBlock.append(title, subtitle);
  header.append(titleBlock, closeButton);
  panel.append(header, layout);
  backdrop.append(panel);
  document.body.append(backdrop);

  let currentGameId: string | null = null;
  let currentGame: GameResponse | null = null;
  let currentAvailableLanguages: string[] = [...DEFAULT_LANGUAGE_OPTIONS];
  let currentAvailableVersionOptions = cloneGameVersionOptions(DEFAULT_GAME_VERSION_BETA_OPTIONS);
  let currentAvailableVersionOptionsWarning = "";
  let currentInstallationDetails: GameInstallationDetails | null = null;
  let currentTab: GamePropertiesTabId = "general";
  let currentGeneralSettings = cloneGeneralSettings(DEFAULT_GENERAL_SETTINGS);
  let currentCompatibilitySettings = cloneCompatibilitySettings(DEFAULT_COMPATIBILITY_SETTINGS);
  let currentControllerSettings = cloneControllerSettings(DEFAULT_CONTROLLER_SETTINGS);
  let currentPrivacySettings = clonePrivacySettings(DEFAULT_PRIVACY_SETTINGS);
  let currentGameVersionsBetasSettings = cloneGameVersionsBetasSettings(DEFAULT_GAME_VERSIONS_BETAS_SETTINGS);
  let currentUpdatesSettings = cloneUpdatesSettings(DEFAULT_UPDATES_SETTINGS);
  let currentSaveSettings: ((settings: GamePropertiesPersistedSettings) => Promise<void>) | null = null;
  let currentBrowseInstalledFiles: (() => Promise<void>) | null = null;
  let currentBackupInstalledFiles: (() => Promise<void>) | null = null;
  let currentVerifyInstalledFiles: (() => Promise<void>) | null = null;
  let currentSetPrivacySettings: ((settings: Pick<GamePrivacySettings, "hideInLibrary" | "markAsPrivate">) => Promise<void>) | null = null;
  let currentDeleteOverlayData: (() => Promise<void>) | null = null;
  let currentInstalledFilesStatus: InstalledFilesStatus = {
    kind: "idle",
    message: "",
  };
  let currentPrivacyStatus: PrivacyStatus = {
    kind: "idle",
    message: "",
  };
  let currentValidateBetaAccessCode: ((accessCode: string) => Promise<GameBetaAccessCodeValidationResult>) | null = null;
  let currentBetaValidationStatus: BetaValidationStatus = {
    kind: "idle",
    message: "",
  };
  let persistDebounceTimeoutId: number | null = null;
  let persistRequestSequence = 0;
  let lastFocusedElement: HTMLElement | null = null;
  let renderCleanupCallbacks: Array<() => void> = [];

  const tabButtons = new Map<GamePropertiesTabId, HTMLButtonElement>();

  const cleanupRenderCallbacks = (): void => {
    for (const callback of renderCleanupCallbacks) {
      callback();
    }

    renderCleanupCallbacks = [];
  };

  const registerRenderCleanup = (callback: () => void): void => {
    renderCleanupCallbacks.push(callback);
  };

  const persistCurrentSettings = (): void => {
    if (!currentSaveSettings) {
      return;
    }

    const settingsSnapshot: GamePropertiesPersistedSettings = {
      compatibility: cloneCompatibilitySettings(currentCompatibilitySettings),
      controller: cloneControllerSettings(currentControllerSettings),
      gameVersionsBetas: cloneGameVersionsBetasSettings(currentGameVersionsBetasSettings),
      general: cloneGeneralSettings(currentGeneralSettings),
      updates: cloneUpdatesSettings(currentUpdatesSettings),
    };

    if (persistDebounceTimeoutId !== null) {
      window.clearTimeout(persistDebounceTimeoutId);
    }

    const requestSequence = ++persistRequestSequence;
    persistDebounceTimeoutId = window.setTimeout(() => {
      persistDebounceTimeoutId = null;
      void currentSaveSettings?.(settingsSnapshot).catch((error: unknown) => {
        if (requestSequence !== persistRequestSequence) {
          return;
        }
        console.error("Failed to persist game properties settings", error);
      });
    }, 180);
  };

  const setTab = (tabId: GamePropertiesTabId): void => {
    if (currentTab === tabId) {
      return;
    }

    currentTab = tabId;
    renderTabContent();
  };

  const moveTabSelection = (originTabId: GamePropertiesTabId, delta: number): void => {
    const activeIndex = GAME_PROPERTIES_TABS.findIndex((tab) => tab.id === originTabId);
    if (activeIndex < 0) {
      return;
    }

    const nextIndex = (activeIndex + delta + GAME_PROPERTIES_TABS.length) % GAME_PROPERTIES_TABS.length;
    const nextTabId = GAME_PROPERTIES_TABS[nextIndex].id;
    setTab(nextTabId);
    tabButtons.get(nextTabId)?.focus();
  };

  const getTabLabel = (tabId: GamePropertiesTabId): string => {
    return GAME_PROPERTIES_TABS.find((tab) => tab.id === tabId)?.label ?? "Properties";
  };

  const resolveDisplayedBuildId = (): string => {
    const selectedOption = currentAvailableVersionOptions.find((option) => {
      return option.id.toLowerCase() === currentGameVersionsBetasSettings.selectedVersionId.toLowerCase();
    });
    if (selectedOption?.buildId) {
      return selectedOption.buildId;
    }

    const defaultOption = currentAvailableVersionOptions.find((option) => {
      return option.isDefault === true || option.id.toLowerCase() === "public";
    });
    if (defaultOption?.buildId) {
      return defaultOption.buildId;
    }

    return "Unavailable";
  };

  const createCustomDropdown = (args: {
    labelledBy: string;
    menuId: string;
    onChange: (value: string) => void;
    options: readonly DropdownOption[];
    selectedValue: string;
    triggerId: string;
  }): {
    closeMenu: () => void;
    field: HTMLDivElement;
    setValue: (value: string, notifyChange?: boolean) => void;
    trigger: HTMLButtonElement;
  } => {
    const {
      labelledBy,
      menuId,
      onChange,
      options,
      selectedValue,
      triggerId,
    } = args;

    const field = document.createElement("div");
    field.className = "game-properties-language-select game-properties-updates-select";

    const trigger = document.createElement("button");
    trigger.id = triggerId;
    trigger.type = "button";
    trigger.className = "game-properties-language-trigger game-properties-updates-trigger text-input";
    trigger.setAttribute("aria-haspopup", "listbox");
    trigger.setAttribute("aria-expanded", "false");
    trigger.setAttribute("aria-labelledby", `${labelledBy} ${trigger.id}`);

    const triggerCopy = document.createElement("span");
    triggerCopy.className = "game-properties-updates-trigger-copy";

    const triggerPrimary = document.createElement("span");
    triggerPrimary.className = "game-properties-updates-trigger-primary";

    const triggerSecondary = document.createElement("span");
    triggerSecondary.className = "game-properties-updates-trigger-secondary";

    const triggerCaret = document.createElement("span");
    triggerCaret.className = "game-properties-language-caret";
    triggerCaret.setAttribute("aria-hidden", "true");

    triggerCopy.append(triggerPrimary, triggerSecondary);

    const menu = document.createElement("div");
    menu.id = menuId;
    menu.className = "game-properties-language-menu game-properties-updates-menu";
    menu.setAttribute("role", "listbox");
    menu.hidden = true;

    trigger.setAttribute("aria-controls", menu.id);
    trigger.append(triggerCopy, triggerCaret);
    field.append(trigger, menu);

    const optionByValue = new Map(options.map((option) => [option.value, option]));
    const optionButtons: HTMLButtonElement[] = [];
    for (const option of options) {
      const optionButton = document.createElement("button");
      optionButton.type = "button";
      optionButton.className = "game-properties-language-option game-properties-updates-option";
      optionButton.setAttribute("role", "option");
      optionButton.dataset.value = option.value;
      optionButton.classList.toggle("has-divider-before", option.dividerBefore === true);

      const optionPrimary = document.createElement("span");
      optionPrimary.className = "game-properties-updates-option-primary";
      optionPrimary.textContent = option.label;

      const optionSecondary = document.createElement("span");
      optionSecondary.className = "game-properties-updates-option-secondary";
      optionSecondary.textContent = option.description ?? "";
      optionSecondary.hidden = !option.description;

      optionButton.append(optionPrimary, optionSecondary);
      menu.append(optionButton);
      optionButtons.push(optionButton);
    }

    const closeMenu = (): void => {
      menu.hidden = true;
      field.classList.remove("is-open");
      trigger.setAttribute("aria-expanded", "false");
    };

    const openMenu = (): void => {
      if (trigger.disabled) {
        return;
      }

      menu.hidden = false;
      field.classList.add("is-open");
      trigger.setAttribute("aria-expanded", "true");
    };

    const focusCurrentOption = (): void => {
      const selectedOption = optionButtons.find((optionButton) => optionButton.classList.contains("is-selected"))
        ?? optionButtons[0];
      selectedOption?.focus();
    };

    const setValue = (value: string, notifyChange = true): void => {
      const selectedOption = optionByValue.get(value) ?? options[0];
      if (!selectedOption) {
        return;
      }

      triggerPrimary.textContent = selectedOption.label;
      triggerSecondary.textContent = selectedOption.description ?? "";
      triggerSecondary.hidden = !selectedOption.description;
      for (const optionButton of optionButtons) {
        const isSelected = optionButton.dataset.value === selectedOption.value;
        optionButton.classList.toggle("is-selected", isSelected);
        optionButton.setAttribute("aria-selected", `${isSelected}`);
      }

      if (!notifyChange) {
        return;
      }

      onChange(selectedOption.value);
    };

    setValue(selectedValue, false);

    for (const optionButton of optionButtons) {
      optionButton.addEventListener("click", () => {
        const optionValue = optionButton.dataset.value;
        if (!optionValue) {
          return;
        }

        setValue(optionValue);
        closeMenu();
        trigger.focus();
      });
    }

    trigger.addEventListener("click", () => {
      if (menu.hidden) {
        openMenu();
        focusCurrentOption();
        return;
      }

      closeMenu();
    });

    trigger.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        if (!menu.hidden) {
          event.preventDefault();
          event.stopPropagation();
          closeMenu();
        }
        return;
      }

      if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        openMenu();
        focusCurrentOption();
      }
    });

    menu.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        closeMenu();
        trigger.focus();
        return;
      }

      if (event.key === "Tab") {
        closeMenu();
        return;
      }

      const activeElement = document.activeElement;
      if (!(activeElement instanceof HTMLButtonElement)) {
        return;
      }

      const focusedIndex = optionButtons.indexOf(activeElement);
      if (focusedIndex < 0) {
        return;
      }

      if (event.key === "ArrowDown") {
        event.preventDefault();
        const nextIndex = Math.min(focusedIndex + 1, optionButtons.length - 1);
        optionButtons[nextIndex].focus();
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        const previousIndex = Math.max(focusedIndex - 1, 0);
        optionButtons[previousIndex].focus();
        return;
      }

      if (event.key === "Home") {
        event.preventDefault();
        optionButtons[0].focus();
        return;
      }

      if (event.key === "End") {
        event.preventDefault();
        optionButtons[optionButtons.length - 1].focus();
        return;
      }

      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        activeElement.click();
      }
    });

    field.addEventListener("focusout", (event) => {
      const relatedTarget = event.relatedTarget;
      if (relatedTarget instanceof Node && field.contains(relatedTarget)) {
        return;
      }

      closeMenu();
    });

    const handlePanelPointerDown = (event: PointerEvent): void => {
      const eventTarget = event.target;
      if (eventTarget instanceof Node && !field.contains(eventTarget)) {
        closeMenu();
      }
    };

    panel.addEventListener("pointerdown", handlePanelPointerDown);
    registerRenderCleanup(() => {
      panel.removeEventListener("pointerdown", handlePanelPointerDown);
    });

    return {
      closeMenu,
      field,
      setValue,
      trigger,
    };
  };

  const renderGeneralTab = (): void => {
    if (!currentGame) {
      return;
    }

    const heading = document.createElement("h3");
    heading.className = "game-properties-section-title";
    heading.textContent = "General";

    const description = document.createElement("p");
    description.className = "game-properties-section-description";
    description.textContent = "Configure launch and in-game behavior for this title.";

    const form = document.createElement("div");
    form.className = "game-properties-form";

    const overlayField = document.createElement("div");
    overlayField.className = "game-properties-field game-properties-switch-field";

    const overlayCopy = document.createElement("div");
    overlayCopy.className = "game-properties-switch-copy";

    const overlayLabelText = document.createElement("p");
    overlayLabelText.className = "game-properties-switch-label";
    overlayLabelText.textContent = "Enable the Steam Overlay while in-game";

    const overlayHint = document.createElement("p");
    overlayHint.className = "game-properties-field-hint";
    overlayHint.textContent = "Lets you access the overlay in supported games.";

    overlayCopy.append(overlayLabelText, overlayHint);

    const overlayToggle = document.createElement("label");
    overlayToggle.className = "game-properties-switch-control";

    const overlayInput = document.createElement("input");
    overlayInput.id = "game-properties-overlay-toggle";
    overlayInput.className = "game-properties-switch-input";
    overlayInput.type = "checkbox";
    overlayInput.checked = currentGeneralSettings.steamOverlayEnabled;
    overlayInput.setAttribute("aria-label", "Enable the Steam Overlay while in-game");

    const overlayTrack = document.createElement("span");
    overlayTrack.className = "game-properties-switch-track";
    overlayTrack.setAttribute("aria-hidden", "true");

    overlayToggle.append(overlayInput, overlayTrack);
    overlayField.append(overlayCopy, overlayToggle);

    const languageField = document.createElement("div");
    languageField.className = "game-properties-field";

    const languageLabel = document.createElement("label");
    languageLabel.className = "game-properties-field-label";
    languageLabel.setAttribute("for", "game-properties-language-trigger");
    languageLabel.textContent = "Language";
    languageLabel.id = "game-properties-language-label";

    const languageSelectField = document.createElement("div");
    languageSelectField.className = "game-properties-language-select";

    const languageTrigger = document.createElement("button");
    languageTrigger.id = "game-properties-language-trigger";
    languageTrigger.type = "button";
    languageTrigger.className = "game-properties-language-trigger text-input";
    languageTrigger.setAttribute("aria-haspopup", "listbox");
    languageTrigger.setAttribute("aria-expanded", "false");
    languageTrigger.setAttribute("aria-labelledby", `${languageLabel.id} ${languageTrigger.id}`);

    const languageTriggerText = document.createElement("span");
    languageTriggerText.className = "game-properties-language-trigger-text";

    const languageTriggerCaret = document.createElement("span");
    languageTriggerCaret.className = "game-properties-language-caret";
    languageTriggerCaret.setAttribute("aria-hidden", "true");

    const languageMenu = document.createElement("div");
    languageMenu.id = "game-properties-language-menu";
    languageMenu.className = "game-properties-language-menu";
    languageMenu.setAttribute("role", "listbox");
    languageMenu.hidden = true;

    languageTrigger.setAttribute("aria-controls", languageMenu.id);
    languageTrigger.append(languageTriggerText, languageTriggerCaret);
    languageSelectField.append(languageTrigger, languageMenu);

    const languageValues = resolveLanguageOptions(
      currentAvailableLanguages,
      currentGeneralSettings.language
    );
    const languageOptionButtons: HTMLButtonElement[] = [];
    for (const language of languageValues) {
      const optionButton = document.createElement("button");
      optionButton.type = "button";
      optionButton.className = "game-properties-language-option";
      optionButton.setAttribute("role", "option");
      optionButton.dataset.value = language;
      optionButton.textContent = language;
      languageMenu.append(optionButton);
      languageOptionButtons.push(optionButton);
    }

    const closeLanguageMenu = (): void => {
      languageMenu.hidden = true;
      languageSelectField.classList.remove("is-open");
      languageTrigger.setAttribute("aria-expanded", "false");
    };

    const openLanguageMenu = (): void => {
      languageMenu.hidden = false;
      languageSelectField.classList.add("is-open");
      languageTrigger.setAttribute("aria-expanded", "true");
    };

    const focusCurrentLanguageOption = (): void => {
      const selectedOption = languageOptionButtons.find((optionButton) => optionButton.classList.contains("is-selected"))
        ?? languageOptionButtons[0];
      selectedOption?.focus();
    };

    const setLanguageValue = (language: string, notifyChange = true): void => {
      const selectedOption = languageOptionButtons.find((optionButton) => optionButton.dataset.value === language);
      if (!selectedOption) {
        return;
      }

      languageTriggerText.textContent = selectedOption.textContent?.trim() ?? language;
      for (const optionButton of languageOptionButtons) {
        const isSelected = optionButton === selectedOption;
        optionButton.classList.toggle("is-selected", isSelected);
        optionButton.setAttribute("aria-selected", `${isSelected}`);
      }

      if (!notifyChange) {
        return;
      }

      currentGeneralSettings = {
        ...currentGeneralSettings,
        language,
      };
      persistCurrentSettings();
    };

    setLanguageValue(currentGeneralSettings.language, false);

    for (const optionButton of languageOptionButtons) {
      optionButton.addEventListener("click", () => {
        const optionValue = optionButton.dataset.value;
        if (!optionValue) {
          return;
        }

        setLanguageValue(optionValue);
        closeLanguageMenu();
        languageTrigger.focus();
      });
    }

    languageTrigger.addEventListener("click", () => {
      if (languageMenu.hidden) {
        openLanguageMenu();
        focusCurrentLanguageOption();
        return;
      }

      closeLanguageMenu();
    });

    languageTrigger.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        if (!languageMenu.hidden) {
          event.preventDefault();
          event.stopPropagation();
          closeLanguageMenu();
        }
        return;
      }

      if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        openLanguageMenu();
        focusCurrentLanguageOption();
      }
    });

    languageMenu.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        closeLanguageMenu();
        languageTrigger.focus();
        return;
      }

      if (event.key === "Tab") {
        closeLanguageMenu();
        return;
      }

      const activeElement = document.activeElement;
      if (!(activeElement instanceof HTMLButtonElement)) {
        return;
      }

      const focusedIndex = languageOptionButtons.indexOf(activeElement);
      if (focusedIndex < 0) {
        return;
      }

      if (event.key === "ArrowDown") {
        event.preventDefault();
        const nextIndex = Math.min(focusedIndex + 1, languageOptionButtons.length - 1);
        languageOptionButtons[nextIndex].focus();
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        const previousIndex = Math.max(focusedIndex - 1, 0);
        languageOptionButtons[previousIndex].focus();
        return;
      }

      if (event.key === "Home") {
        event.preventDefault();
        languageOptionButtons[0].focus();
        return;
      }

      if (event.key === "End") {
        event.preventDefault();
        languageOptionButtons[languageOptionButtons.length - 1].focus();
        return;
      }

      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        activeElement.click();
      }
    });

    languageSelectField.addEventListener("focusout", (event) => {
      const relatedTarget = event.relatedTarget;
      if (relatedTarget instanceof Node && languageSelectField.contains(relatedTarget)) {
        return;
      }

      closeLanguageMenu();
    });

    const handlePanelPointerDown = (event: PointerEvent): void => {
      const eventTarget = event.target;
      if (eventTarget instanceof Node && !languageSelectField.contains(eventTarget)) {
        closeLanguageMenu();
      }
    };

    panel.addEventListener("pointerdown", handlePanelPointerDown);
    registerRenderCleanup(() => {
      panel.removeEventListener("pointerdown", handlePanelPointerDown);
    });

    languageField.append(languageLabel, languageSelectField);

    const launchOptionsField = document.createElement("div");
    launchOptionsField.className = "game-properties-field";

    const launchOptionsLabel = document.createElement("label");
    launchOptionsLabel.className = "game-properties-field-label";
    launchOptionsLabel.setAttribute("for", "game-properties-launch-options");
    launchOptionsLabel.textContent = "Launch Options";

    const launchOptions = document.createElement("textarea");
    launchOptions.id = "game-properties-launch-options";
    launchOptions.className = "game-properties-launch-options text-input";
    launchOptions.placeholder = "e.g. -novid -windowed";
    launchOptions.value = currentGeneralSettings.launchOptions;
    launchOptions.rows = 3;

    const launchOptionsHint = document.createElement("p");
    launchOptionsHint.className = "game-properties-field-hint";
    launchOptionsHint.textContent = "Add command-line arguments to apply whenever this game launches.";

    launchOptionsField.append(launchOptionsLabel, launchOptions, launchOptionsHint);
    form.append(overlayField, languageField, launchOptionsField);
    tabPanel.append(heading, description, form);

    overlayInput.addEventListener("change", () => {
      currentGeneralSettings = {
        ...currentGeneralSettings,
        steamOverlayEnabled: overlayInput.checked,
      };
      persistCurrentSettings();
    });

    launchOptions.addEventListener("input", () => {
      currentGeneralSettings = {
        ...currentGeneralSettings,
        launchOptions: launchOptions.value,
      };
      persistCurrentSettings();
    });
  };

  const renderCompatibilityTab = (): void => {
    if (!currentGame) {
      return;
    }

    const isSteamGame = currentGame.provider.trim().toLowerCase() === "steam";
    const heading = document.createElement("h3");
    heading.className = "game-properties-section-title";
    heading.textContent = "Compatibility";

    const description = document.createElement("p");
    description.className = "game-properties-section-description";
    description.textContent = "Control Steam Play compatibility behavior for this title.";

    const form = document.createElement("div");
    form.className = "game-properties-form";

    const forceToolField = document.createElement("div");
    forceToolField.className = "game-properties-field game-properties-switch-field";

    const forceToolCopy = document.createElement("div");
    forceToolCopy.className = "game-properties-switch-copy";

    const forceToolLabelText = document.createElement("p");
    forceToolLabelText.className = "game-properties-switch-label";
    forceToolLabelText.textContent = "Force the use of a specific Steam Play compatibility tool";

    const forceToolHint = document.createElement("p");
    forceToolHint.className = "game-properties-field-hint";
    forceToolHint.textContent = isSteamGame
      ? "Enable this to choose a specific Proton or Steam runtime tool for launches."
      : "Only available for Steam games.";

    forceToolCopy.append(forceToolLabelText, forceToolHint);

    const forceToolToggle = document.createElement("label");
    forceToolToggle.className = "game-properties-switch-control";

    const forceToolInput = document.createElement("input");
    forceToolInput.id = "game-properties-force-compatibility-tool";
    forceToolInput.className = "game-properties-switch-input";
    forceToolInput.type = "checkbox";
    forceToolInput.checked = isSteamGame && currentCompatibilitySettings.forceSteamPlayCompatibilityTool;
    forceToolInput.disabled = !isSteamGame;
    forceToolInput.setAttribute(
      "aria-label",
      "Force the use of a specific Steam Play compatibility tool"
    );

    const forceToolTrack = document.createElement("span");
    forceToolTrack.className = "game-properties-switch-track";
    forceToolTrack.setAttribute("aria-hidden", "true");

    forceToolToggle.append(forceToolInput, forceToolTrack);
    forceToolField.append(forceToolCopy, forceToolToggle);

    const toolField = document.createElement("div");
    toolField.className = "game-properties-field";
    toolField.hidden = !isSteamGame || !currentCompatibilitySettings.forceSteamPlayCompatibilityTool;

    const toolLabel = document.createElement("label");
    toolLabel.className = "game-properties-field-label";
    toolLabel.textContent = "Steam Play Compatibility Tool";
    toolLabel.setAttribute("for", "game-properties-compatibility-tool-trigger");
    toolLabel.id = "game-properties-compatibility-tool-label";

    const toolSelectField = document.createElement("div");
    toolSelectField.className = "game-properties-language-select";

    const toolTrigger = document.createElement("button");
    toolTrigger.id = "game-properties-compatibility-tool-trigger";
    toolTrigger.type = "button";
    toolTrigger.className = "game-properties-language-trigger text-input";
    toolTrigger.setAttribute("aria-haspopup", "listbox");
    toolTrigger.setAttribute("aria-expanded", "false");
    toolTrigger.setAttribute("aria-labelledby", `${toolLabel.id} ${toolTrigger.id}`);

    const toolTriggerText = document.createElement("span");
    toolTriggerText.className = "game-properties-language-trigger-text";

    const toolTriggerCaret = document.createElement("span");
    toolTriggerCaret.className = "game-properties-language-caret";
    toolTriggerCaret.setAttribute("aria-hidden", "true");

    const toolMenu = document.createElement("div");
    toolMenu.id = "game-properties-compatibility-tool-menu";
    toolMenu.className = "game-properties-language-menu";
    toolMenu.setAttribute("role", "listbox");
    toolMenu.hidden = true;

    toolTrigger.setAttribute("aria-controls", toolMenu.id);
    toolTrigger.append(toolTriggerText, toolTriggerCaret);
    toolSelectField.append(toolTrigger, toolMenu);

    const compatibilityToolValues = resolveCompatibilityToolOptions(
      currentCompatibilitySettings.steamPlayCompatibilityTool
    );
    const compatibilityOptionButtons: HTMLButtonElement[] = [];
    for (const tool of compatibilityToolValues) {
      const optionButton = document.createElement("button");
      optionButton.type = "button";
      optionButton.className = "game-properties-language-option";
      optionButton.setAttribute("role", "option");
      optionButton.dataset.value = tool;
      optionButton.textContent = tool;
      toolMenu.append(optionButton);
      compatibilityOptionButtons.push(optionButton);
    }

    const closeToolMenu = (): void => {
      toolMenu.hidden = true;
      toolSelectField.classList.remove("is-open");
      toolTrigger.setAttribute("aria-expanded", "false");
    };

    const openToolMenu = (): void => {
      if (toolTrigger.disabled) {
        return;
      }

      toolMenu.hidden = false;
      toolSelectField.classList.add("is-open");
      toolTrigger.setAttribute("aria-expanded", "true");
    };

    const focusCurrentToolOption = (): void => {
      const selectedOption = compatibilityOptionButtons.find((optionButton) => optionButton.classList.contains("is-selected"))
        ?? compatibilityOptionButtons[0];
      selectedOption?.focus();
    };

    const setCompatibilityTool = (tool: string, notifyChange = true): void => {
      const selectedOption = compatibilityOptionButtons.find((optionButton) => optionButton.dataset.value === tool);
      if (!selectedOption) {
        return;
      }

      toolTriggerText.textContent = selectedOption.textContent?.trim() ?? tool;
      for (const optionButton of compatibilityOptionButtons) {
        const isSelected = optionButton === selectedOption;
        optionButton.classList.toggle("is-selected", isSelected);
        optionButton.setAttribute("aria-selected", `${isSelected}`);
      }

      if (!notifyChange) {
        return;
      }

      currentCompatibilitySettings = {
        ...currentCompatibilitySettings,
        steamPlayCompatibilityTool: tool,
      };
      persistCurrentSettings();
    };

    setCompatibilityTool(currentCompatibilitySettings.steamPlayCompatibilityTool, false);

    for (const optionButton of compatibilityOptionButtons) {
      optionButton.addEventListener("click", () => {
        const optionValue = optionButton.dataset.value;
        if (!optionValue) {
          return;
        }

        setCompatibilityTool(optionValue);
        closeToolMenu();
        toolTrigger.focus();
      });
    }

    toolTrigger.addEventListener("click", () => {
      if (toolMenu.hidden) {
        openToolMenu();
        focusCurrentToolOption();
        return;
      }

      closeToolMenu();
    });

    toolTrigger.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        if (!toolMenu.hidden) {
          event.preventDefault();
          event.stopPropagation();
          closeToolMenu();
        }
        return;
      }

      if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        openToolMenu();
        focusCurrentToolOption();
      }
    });

    toolMenu.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        closeToolMenu();
        toolTrigger.focus();
        return;
      }

      if (event.key === "Tab") {
        closeToolMenu();
        return;
      }

      const activeElement = document.activeElement;
      if (!(activeElement instanceof HTMLButtonElement)) {
        return;
      }

      const focusedIndex = compatibilityOptionButtons.indexOf(activeElement);
      if (focusedIndex < 0) {
        return;
      }

      if (event.key === "ArrowDown") {
        event.preventDefault();
        const nextIndex = Math.min(focusedIndex + 1, compatibilityOptionButtons.length - 1);
        compatibilityOptionButtons[nextIndex].focus();
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        const previousIndex = Math.max(focusedIndex - 1, 0);
        compatibilityOptionButtons[previousIndex].focus();
        return;
      }

      if (event.key === "Home") {
        event.preventDefault();
        compatibilityOptionButtons[0].focus();
        return;
      }

      if (event.key === "End") {
        event.preventDefault();
        compatibilityOptionButtons[compatibilityOptionButtons.length - 1].focus();
        return;
      }

      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        activeElement.click();
      }
    });

    toolSelectField.addEventListener("focusout", (event) => {
      const relatedTarget = event.relatedTarget;
      if (relatedTarget instanceof Node && toolSelectField.contains(relatedTarget)) {
        return;
      }

      closeToolMenu();
    });

    const handlePanelPointerDown = (event: PointerEvent): void => {
      const eventTarget = event.target;
      if (eventTarget instanceof Node && !toolSelectField.contains(eventTarget)) {
        closeToolMenu();
      }
    };

    panel.addEventListener("pointerdown", handlePanelPointerDown);
    registerRenderCleanup(() => {
      panel.removeEventListener("pointerdown", handlePanelPointerDown);
    });

    forceToolInput.addEventListener("change", () => {
      currentCompatibilitySettings = {
        ...currentCompatibilitySettings,
        forceSteamPlayCompatibilityTool: forceToolInput.checked,
      };
      toolField.hidden = !forceToolInput.checked;
      closeToolMenu();
      persistCurrentSettings();
    });

    toolField.append(toolLabel, toolSelectField);
    form.append(forceToolField, toolField);
    tabPanel.append(heading, description, form);
  };

  const renderUpdatesTab = (): void => {
    if (!currentGame) {
      return;
    }

    const heading = document.createElement("h3");
    heading.className = "game-properties-section-title";
    heading.textContent = "Updates";

    const form = document.createElement("div");
    form.className = "game-properties-form";

    const automaticUpdatesSection = document.createElement("section");
    automaticUpdatesSection.className = "game-properties-updates-section";

    const automaticUpdatesLabel = document.createElement("label");
    automaticUpdatesLabel.className = "game-properties-field-label";
    automaticUpdatesLabel.id = "game-properties-automatic-updates-label";
    automaticUpdatesLabel.textContent = "Automatic Updates";
    automaticUpdatesLabel.setAttribute("for", "game-properties-automatic-updates-trigger");

    const automaticUpdatesHint = document.createElement("p");
    automaticUpdatesHint.className = "game-properties-field-hint";
    automaticUpdatesHint.textContent = "Choose how this game should receive updates.";

    const automaticUpdatesDropdown = createCustomDropdown({
      labelledBy: automaticUpdatesLabel.id,
      menuId: "game-properties-automatic-updates-menu",
      onChange: (value) => {
        if (!isAutomaticUpdatesMode(value)) {
          return;
        }

        currentUpdatesSettings = {
          ...currentUpdatesSettings,
          automaticUpdatesMode: value,
        };
        persistCurrentSettings();
      },
      options: AUTOMATIC_UPDATES_OPTIONS,
      selectedValue: currentUpdatesSettings.automaticUpdatesMode,
      triggerId: "game-properties-automatic-updates-trigger",
    });

    automaticUpdatesSection.append(
      automaticUpdatesLabel,
      automaticUpdatesHint,
      automaticUpdatesDropdown.field,
    );

    const backgroundDownloadsSection = document.createElement("section");
    backgroundDownloadsSection.className = "game-properties-updates-section";

    const backgroundDownloadsLabel = document.createElement("label");
    backgroundDownloadsLabel.className = "game-properties-field-label";
    backgroundDownloadsLabel.id = "game-properties-background-downloads-label";
    backgroundDownloadsLabel.textContent = "Background Downloads";
    backgroundDownloadsLabel.setAttribute("for", "game-properties-background-downloads-trigger");

    const backgroundDownloadsHint = document.createElement("p");
    backgroundDownloadsHint.className = "game-properties-field-hint";
    backgroundDownloadsHint.textContent = "While playing, should Steam be allowed to download other updates?";

    const backgroundDownloadsDropdown = createCustomDropdown({
      labelledBy: backgroundDownloadsLabel.id,
      menuId: "game-properties-background-downloads-menu",
      onChange: (value) => {
        if (!isBackgroundDownloadsMode(value)) {
          return;
        }

        currentUpdatesSettings = {
          ...currentUpdatesSettings,
          backgroundDownloadsMode: value,
        };
        persistCurrentSettings();
      },
      options: BACKGROUND_DOWNLOAD_OPTIONS,
      selectedValue: currentUpdatesSettings.backgroundDownloadsMode,
      triggerId: "game-properties-background-downloads-trigger",
    });

    backgroundDownloadsSection.append(
      backgroundDownloadsLabel,
      backgroundDownloadsHint,
      backgroundDownloadsDropdown.field,
    );

    const metadata = document.createElement("div");
    metadata.className = "game-properties-updates-meta";

    const appId = document.createElement("p");
    appId.textContent = `App ID: ${currentGame.externalId}`;

    const buildId = document.createElement("p");
    buildId.textContent = `Build ID: ${resolveDisplayedBuildId()}`;

    const installedUpdatedAt = document.createElement("p");
    installedUpdatedAt.textContent = `Installed content updated: ${formatTimestampForMetadata(currentGame.lastSyncedAt)}`;

    metadata.append(appId, buildId, installedUpdatedAt);
    form.append(automaticUpdatesSection, backgroundDownloadsSection);
    tabPanel.append(heading, form, metadata);
  };

  const renderInstalledFilesTab = (): void => {
    if (!currentGame) {
      return;
    }
    const renderedGame = currentGame;

    const heading = document.createElement("h3");
    heading.className = "game-properties-section-title";
    heading.textContent = "Installed Files";

    const container = document.createElement("div");
    container.className = "game-properties-installed-files";

    const sizeRow = document.createElement("section");
    sizeRow.className = "game-properties-installed-row";

    const sizeCopy = document.createElement("div");
    sizeCopy.className = "game-properties-installed-copy";

    const sizeText = document.createElement("p");
    sizeText.className = "game-properties-installed-title";
    let installPathButton: HTMLButtonElement | null = null;
    if (renderedGame.installed) {
      const installPath = currentInstallationDetails?.installPath?.trim() ?? "";
      const sizeOnDiskBytes = currentInstallationDetails?.sizeOnDiskBytes;
      sizeText.textContent = "Size of installation: ";
      const sizeValue = document.createElement("span");
      sizeValue.className = "game-properties-installed-emphasis";
      sizeValue.textContent = typeof sizeOnDiskBytes === "number"
        ? formatSizeForMetadata(sizeOnDiskBytes)
        : "Unknown size";
      sizeText.append(sizeValue);
      if (installPath.length > 0) {
        sizeText.append(" on ");
        installPathButton = document.createElement("button");
        installPathButton.type = "button";
        installPathButton.className = "game-properties-installed-path-link";
        installPathButton.textContent = installPath;
        installPathButton.setAttribute("aria-label", `Open install path ${installPath}`);
        sizeText.append(installPathButton);
      }
    } else {
      sizeText.textContent = "Size of installation: Not currently installed";
    }

    sizeCopy.append(sizeText);

    const browseButton = document.createElement("button");
    browseButton.type = "button";
    browseButton.className = "game-properties-installed-action";
    browseButton.textContent = "Browse...";
    browseButton.disabled = !renderedGame.installed;

    sizeRow.append(sizeCopy, browseButton);

    const backupRow = document.createElement("section");
    backupRow.className = "game-properties-installed-row";

    const backupCopy = document.createElement("div");
    backupCopy.className = "game-properties-installed-copy";

    const backupText = document.createElement("p");
    backupText.className = "game-properties-installed-title";
    backupText.textContent = "Create a backup of the installed files to restore this game in the future";

    const backupFaq = document.createElement("a");
    backupFaq.className = "game-properties-installed-link";
    backupFaq.href = "https://help.steampowered.com/en/faqs/";
    backupFaq.target = "_blank";
    backupFaq.rel = "noreferrer noopener";
    backupFaq.textContent = "Read the FAQ";

    backupCopy.append(backupText, backupFaq);

    const backupButton = document.createElement("button");
    backupButton.type = "button";
    backupButton.className = "game-properties-installed-action";
    backupButton.textContent = "Backup game files";
    backupButton.disabled = !renderedGame.installed;

    backupRow.append(backupCopy, backupButton);

    const verifyRow = document.createElement("section");
    verifyRow.className = "game-properties-installed-row";

    const verifyCopy = document.createElement("div");
    verifyCopy.className = "game-properties-installed-copy";

    const verifyText = document.createElement("p");
    verifyText.className = "game-properties-installed-title";
    verifyText.textContent = "Verify this game's files are installed correctly";

    const verifyFaq = document.createElement("a");
    verifyFaq.className = "game-properties-installed-link";
    verifyFaq.href = "https://help.steampowered.com/en/faqs/";
    verifyFaq.target = "_blank";
    verifyFaq.rel = "noreferrer noopener";
    verifyFaq.textContent = "Read the FAQ";

    verifyCopy.append(verifyText, verifyFaq);

    const verifyButton = document.createElement("button");
    verifyButton.type = "button";
    verifyButton.className = "game-properties-installed-action";
    verifyButton.textContent = "Verify integrity of game files";
    verifyButton.disabled = !renderedGame.installed;

    verifyRow.append(verifyCopy, verifyButton);

    const metadata = document.createElement("div");
    metadata.className = "game-properties-installed-meta";

    const appId = document.createElement("p");
    appId.textContent = `App ID: ${renderedGame.externalId}`;

    const buildId = document.createElement("p");
    buildId.textContent = `Build ID: ${resolveDisplayedBuildId()}`;

    const installedUpdatedAt = document.createElement("p");
    installedUpdatedAt.textContent = `Installed content updated: ${formatTimestampForMetadata(renderedGame.lastSyncedAt)}`;

    const installedStatus = document.createElement("p");
    installedStatus.className = "game-properties-privacy-feedback";
    const applyInstalledStatus = (status: InstalledFilesStatus): void => {
      currentInstalledFilesStatus = status;
      installedStatus.hidden = status.message.trim().length === 0;
      installedStatus.textContent = status.message;
      installedStatus.classList.toggle("is-success", status.kind === "success");
      installedStatus.classList.toggle("is-error", status.kind === "error");
    };
    applyInstalledStatus(currentInstalledFilesStatus);

    let isActionInProgress = false;
    const setInstalledActionBusyState = (isBusy: boolean): void => {
      isActionInProgress = isBusy;
      browseButton.disabled = isBusy || !renderedGame.installed;
      backupButton.disabled = isBusy || !renderedGame.installed;
      verifyButton.disabled = isBusy || !renderedGame.installed;
      if (installPathButton) {
        installPathButton.disabled = isBusy || !renderedGame.installed;
      }
    };

    const runInstalledAction = async (
      action: (() => Promise<void>) | null,
      successMessage: string
    ): Promise<void> => {
      if (isActionInProgress) {
        return;
      }

      if (!renderedGame.installed) {
        applyInstalledStatus({
          kind: "error",
          message: "This game is not currently installed.",
        });
        return;
      }

      if (!action) {
        applyInstalledStatus({
          kind: "error",
          message: "This action is unavailable right now.",
        });
        return;
      }

      setInstalledActionBusyState(true);
      try {
        await action();
        applyInstalledStatus({
          kind: "success",
          message: successMessage,
        });
      } catch {
        applyInstalledStatus({
          kind: "error",
          message: "Could not complete this action right now.",
        });
      } finally {
        setInstalledActionBusyState(false);
      }
    };

    browseButton.addEventListener("click", () => {
      void runInstalledAction(
        currentBrowseInstalledFiles,
        "Opened game install folder."
      );
    });
    installPathButton?.addEventListener("click", () => {
      void runInstalledAction(
        currentBrowseInstalledFiles,
        "Opened game install folder."
      );
    });

    backupButton.addEventListener("click", () => {
      void runInstalledAction(
        currentBackupInstalledFiles,
        "Opened Steam backup flow."
      );
    });

    verifyButton.addEventListener("click", () => {
      void runInstalledAction(
        currentVerifyInstalledFiles,
        "Started Steam file verification."
      );
    });

    metadata.append(appId, buildId, installedUpdatedAt);
    container.append(sizeRow, backupRow, verifyRow, metadata, installedStatus);
    tabPanel.append(heading, container);
  };

  const renderGameVersionsBetasTab = (): void => {
    const heading = document.createElement("h3");
    heading.className = "game-properties-section-title";
    heading.textContent = "Game Versions & Betas";

    const selectedVersionHeader = document.createElement("div");
    selectedVersionHeader.className = "game-properties-inline-label-row";

    const selectedVersionTitle = document.createElement("p");
    selectedVersionTitle.className = "game-properties-field-label";
    selectedVersionTitle.textContent = "Selected Game Version";
    selectedVersionHeader.append(selectedVersionTitle);

    const trimmedWarning = currentAvailableVersionOptionsWarning.trim();
    if (trimmedWarning.length > 0) {
      const warningTooltip = document.createElement("button");
      warningTooltip.type = "button";
      warningTooltip.className = "game-properties-info-tooltip";
      warningTooltip.textContent = "i";
      warningTooltip.setAttribute("aria-label", trimmedWarning);
      warningTooltip.title = trimmedWarning;
      selectedVersionHeader.append(warningTooltip);
    }

    const selectedVersionHint = document.createElement("p");
    selectedVersionHint.className = "game-properties-section-description";
    selectedVersionHint.textContent = "Versions here are provided by the game developer and may be unstable test builds or old versions of the game. Proceed with caution.";

    const versionsTable = document.createElement("div");
    versionsTable.className = "game-properties-versions-table";

    const versionsTableHead = document.createElement("div");
    versionsTableHead.className = "game-properties-versions-head";

    const headName = document.createElement("span");
    headName.textContent = "Name & Description";

    const headUpdated = document.createElement("span");
    headUpdated.textContent = "Last Updated";

    versionsTableHead.append(headName, headUpdated);

    const versionsRows = document.createElement("div");
    versionsRows.className = "game-properties-versions-rows";
    versionsRows.setAttribute("role", "radiogroup");
    versionsRows.setAttribute("aria-label", "Select game version");

    currentAvailableVersionOptions = resolveGameVersionBetaOptions(
      currentAvailableVersionOptions,
      currentGameVersionsBetasSettings.selectedVersionId
    );

    const versionRadioName = `game-properties-version-${currentGameId ?? "unknown"}`;
    const versionRows: Array<{ id: GameVersionBetaId; radio: HTMLInputElement; row: HTMLLabelElement }> = [];
    const syncSelectedVersion = (selectedVersionId: GameVersionBetaId): void => {
      for (const entry of versionRows) {
        const isSelected = entry.id === selectedVersionId;
        entry.radio.checked = isSelected;
        entry.row.classList.toggle("is-selected", isSelected);
      }
    };

    for (const option of currentAvailableVersionOptions) {
      const rowLabel = document.createElement("label");
      rowLabel.className = "game-properties-versions-row";

      const radio = document.createElement("input");
      radio.type = "radio";
      radio.className = "game-properties-versions-radio";
      radio.name = versionRadioName;
      radio.value = option.id;
      radio.checked = currentGameVersionsBetasSettings.selectedVersionId === option.id;
      radio.setAttribute("aria-label", option.name);

      const radioIndicator = document.createElement("span");
      radioIndicator.className = "game-properties-versions-radio-indicator";
      radioIndicator.setAttribute("aria-hidden", "true");

      const rowCopy = document.createElement("div");
      rowCopy.className = "game-properties-versions-copy";

      const rowName = document.createElement("p");
      rowName.className = "game-properties-versions-name";
      rowName.textContent = option.name;

      const rowNameWithBadges = document.createElement("div");
      rowNameWithBadges.className = "game-properties-versions-name-row";
      rowNameWithBadges.append(rowName);

      const appendBadge = (label: string, modifierClass: string): void => {
        const badge = document.createElement("span");
        badge.className = `game-properties-versions-badge ${modifierClass}`;
        badge.textContent = label;
        rowNameWithBadges.append(badge);
      };
      if (option.isDefault === true) {
        appendBadge("Default", "is-default");
      } else {
        appendBadge("Private", "is-private");
      }

      if (option.requiresAccessCode === true) {
        appendBadge("Requires Code", "is-requires-code");
      }

      const rowDescription = document.createElement("p");
      rowDescription.className = "game-properties-versions-description";
      rowDescription.textContent = option.description;

      const rowUpdated = document.createElement("p");
      rowUpdated.className = "game-properties-versions-updated";
      rowUpdated.textContent = option.lastUpdated;

      rowCopy.append(rowNameWithBadges, rowDescription);
      rowLabel.append(radio, radioIndicator, rowCopy, rowUpdated);
      versionsRows.append(rowLabel);
      versionRows.push({
        id: option.id,
        radio,
        row: rowLabel,
      });

      radio.addEventListener("change", () => {
        if (!radio.checked) {
          return;
        }

        currentGameVersionsBetasSettings = {
          ...currentGameVersionsBetasSettings,
          selectedVersionId: option.id,
        };
        syncSelectedVersion(option.id);
        persistCurrentSettings();
      });
    }

    syncSelectedVersion(currentGameVersionsBetasSettings.selectedVersionId);
    versionsTable.append(versionsTableHead, versionsRows);

    const privateVersionsSection = document.createElement("section");
    privateVersionsSection.className = "game-properties-private-versions";

    const privateVersionsTitle = document.createElement("p");
    privateVersionsTitle.className = "game-properties-field-label";
    privateVersionsTitle.textContent = "Private Versions";

    const privateVersionsHint = document.createElement("p");
    privateVersionsHint.className = "game-properties-field-hint";
    privateVersionsHint.textContent = "Enter access code to unlock a private game version or beta:";

    const privateVersionsActions = document.createElement("div");
    privateVersionsActions.className = "game-properties-private-actions";

    const privateAccessCode = document.createElement("input");
    privateAccessCode.type = "text";
    privateAccessCode.className = "game-properties-private-input text-input";
    privateAccessCode.autocomplete = "off";
    privateAccessCode.value = currentGameVersionsBetasSettings.privateAccessCode;

    const checkCodeButton = document.createElement("button");
    checkCodeButton.type = "button";
    checkCodeButton.className = "game-properties-installed-action";
    checkCodeButton.textContent = "Check Code";

    const validationStatus = document.createElement("p");
    validationStatus.className = "game-properties-validation-status";

    const applyValidationStatus = (status: BetaValidationStatus): void => {
      currentBetaValidationStatus = status;
      validationStatus.classList.toggle("is-loading", status.kind === "loading");
      validationStatus.classList.toggle("is-success", status.kind === "success");
      validationStatus.classList.toggle("is-error", status.kind === "error");
      validationStatus.hidden = status.message.length === 0;
      validationStatus.textContent = status.message;
    };

    applyValidationStatus(currentBetaValidationStatus);
    privateVersionsActions.append(privateAccessCode, checkCodeButton);
    privateVersionsSection.append(privateVersionsTitle, privateVersionsHint, privateVersionsActions, validationStatus);
    tabPanel.append(heading, selectedVersionHeader, selectedVersionHint, versionsTable, privateVersionsSection);

    privateAccessCode.addEventListener("input", () => {
      currentGameVersionsBetasSettings = {
        ...currentGameVersionsBetasSettings,
        privateAccessCode: privateAccessCode.value,
      };
      if (currentBetaValidationStatus.kind !== "idle") {
        applyValidationStatus({
          kind: "idle",
          message: "",
        });
      }
      persistCurrentSettings();
    });

    let isDisposed = false;
    registerRenderCleanup(() => {
      isDisposed = true;
    });

    checkCodeButton.addEventListener("click", async () => {
      const accessCode = privateAccessCode.value.trim();
      if (accessCode.length === 0) {
        applyValidationStatus({
          kind: "error",
          message: "Enter an access code before checking.",
        });
        return;
      }

      if (!currentValidateBetaAccessCode) {
        applyValidationStatus({
          kind: "error",
          message: "Access code validation is unavailable right now.",
        });
        return;
      }

      checkCodeButton.disabled = true;
      applyValidationStatus({
        kind: "loading",
        message: "Checking access code...",
      });

      try {
        const validation = await currentValidateBetaAccessCode(accessCode);
        if (isDisposed) {
          return;
        }

        if (!validation.valid) {
          applyValidationStatus({
            kind: "error",
            message: validation.message.trim().length > 0
              ? validation.message
              : "Code is invalid.",
          });
          return;
        }

        const validatedBranchId = validation.branchId?.trim() ?? "";
        if (validatedBranchId.length > 0) {
          const existingBranch = currentAvailableVersionOptions.find((option) => {
            return option.id.toLowerCase() === validatedBranchId.toLowerCase();
          });
          if (!existingBranch) {
            const unlockedBranchName = validation.branchName?.trim() || validatedBranchId;
            currentAvailableVersionOptions = [
              ...currentAvailableVersionOptions,
              {
                id: validatedBranchId,
                name: unlockedBranchName,
                description: "Unlocked private beta branch",
                lastUpdated: "Unavailable",
                requiresAccessCode: true,
                isDefault: false,
              },
            ];
            currentGameVersionsBetasSettings = {
              ...currentGameVersionsBetasSettings,
              selectedVersionId: validatedBranchId,
            };
            persistCurrentSettings();
            applyValidationStatus({
              kind: "success",
              message: validation.message.trim().length > 0
                ? validation.message
                : `Code accepted. ${unlockedBranchName} is now available.`,
            });
            renderTabContent();
            return;
          }

          currentGameVersionsBetasSettings = {
            ...currentGameVersionsBetasSettings,
            selectedVersionId: existingBranch.id,
          };
          syncSelectedVersion(existingBranch.id);
          persistCurrentSettings();
        }

        applyValidationStatus({
          kind: "success",
          message: validation.message.trim().length > 0
            ? validation.message
            : "Code accepted.",
        });
      } catch {
        if (isDisposed) {
          return;
        }

        applyValidationStatus({
          kind: "error",
          message: "Could not validate this code right now.",
        });
      } finally {
        if (!isDisposed) {
          checkCodeButton.disabled = false;
        }
      }
    });
  };

  const renderControllerTab = (): void => {
    if (!currentGame) {
      return;
    }

    const heading = document.createElement("h3");
    heading.className = "game-properties-section-title";
    heading.textContent = "Controller";

    const description = document.createElement("p");
    description.className = "game-properties-section-description";
    description.textContent = "Steam Input allows any controller to be used with any Steam game and enables controller reconfiguration.";

    const configuratorCopy = document.createElement("p");
    configuratorCopy.className = "game-properties-controller-config-copy";
    configuratorCopy.append("Use the ");

    const configuratorLink = document.createElement("a");
    configuratorLink.className = "game-properties-controller-config-link";
    configuratorLink.href = "https://help.steampowered.com/en/faqs/";
    configuratorLink.target = "_blank";
    configuratorLink.rel = "noreferrer noopener";
    configuratorLink.textContent = "Controller Configurator";

    configuratorCopy.append(configuratorLink, " to see more details or remap your controller.");

    const overrideSection = document.createElement("section");
    overrideSection.className = "game-properties-controller-override";

    const overrideRow = document.createElement("div");
    overrideRow.className = "game-properties-controller-override-row";

    const overrideCopy = document.createElement("div");
    overrideCopy.className = "game-properties-controller-override-copy";

    const overrideTitle = document.createElement("p");
    overrideTitle.className = "game-properties-field-label";
    overrideTitle.textContent = `Override for ${currentGame.name}`;

    const overrideHint = document.createElement("p");
    overrideHint.className = "game-properties-controller-restart-hint";
    overrideHint.textContent = "(Changing requires restart of game)";

    const overrideLabelId = "game-properties-controller-override-label";
    overrideTitle.id = overrideLabelId;

    const overrideDropdown = createCustomDropdown({
      labelledBy: overrideLabelId,
      menuId: "game-properties-controller-override-menu",
      onChange: (value) => {
        if (!isSteamInputOverrideMode(value)) {
          return;
        }

        currentControllerSettings = {
          ...currentControllerSettings,
          steamInputOverride: value,
        };
        persistCurrentSettings();
      },
      options: CONTROLLER_OVERRIDE_OPTIONS,
      selectedValue: currentControllerSettings.steamInputOverride,
      triggerId: "game-properties-controller-override-trigger",
    });
    overrideDropdown.field.classList.add("game-properties-controller-dropdown");

    overrideCopy.append(overrideTitle, overrideHint);
    overrideRow.append(overrideCopy, overrideDropdown.field);
    overrideSection.append(overrideRow);

    const statusPanel = document.createElement("section");
    statusPanel.className = "game-properties-controller-status";

    const statusLabel = document.createElement("p");
    statusLabel.className = "game-properties-controller-status-label";
    statusLabel.textContent = "Steam Input status:";

    const statusTable = document.createElement("div");
    statusTable.className = "game-properties-controller-status-table";

    for (const row of CONTROLLER_STATUS_ROWS) {
      const statusRow = document.createElement("div");
      statusRow.className = "game-properties-controller-status-row";

      const statusName = document.createElement("span");
      statusName.className = "game-properties-controller-status-name";
      statusName.textContent = row.label;

      const statusValue = document.createElement("span");
      statusValue.className = "game-properties-controller-status-value";
      statusValue.textContent = row.status;
      statusValue.classList.toggle("is-empty", row.status.trim().length === 0);

      statusRow.append(statusName, statusValue);
      statusTable.append(statusRow);
    }

    statusPanel.append(statusLabel, statusTable);
    tabPanel.append(heading, description, configuratorCopy, overrideSection, statusPanel);
  };

  const renderPrivacyTab = (): void => {
    const heading = document.createElement("h3");
    heading.className = "game-properties-section-title";
    heading.textContent = "Privacy";

    const section = document.createElement("div");
    section.className = "game-properties-privacy";

    const hideRow = document.createElement("section");
    hideRow.className = "game-properties-privacy-row";

    const hideHeader = document.createElement("div");
    hideHeader.className = "game-properties-privacy-header";

    const hideTitle = document.createElement("p");
    hideTitle.className = "game-properties-privacy-title";
    hideTitle.textContent = "Hide in library";

    const hideToggle = document.createElement("label");
    hideToggle.className = "game-properties-switch-control";

    const hideToggleInput = document.createElement("input");
    hideToggleInput.type = "checkbox";
    hideToggleInput.className = "game-properties-switch-input";
    hideToggleInput.checked = currentPrivacySettings.hideInLibrary;
    hideToggleInput.setAttribute("aria-label", "Hide in library");

    const hideTrack = document.createElement("span");
    hideTrack.className = "game-properties-switch-track";
    hideTrack.setAttribute("aria-hidden", "true");

    hideToggle.append(hideToggleInput, hideTrack);
    hideHeader.append(hideTitle, hideToggle);

    const hideDescription = document.createElement("p");
    hideDescription.className = "game-properties-privacy-description";
    hideDescription.textContent = "Hide this game in the Steam Library. You can access the game by selecting the \"Hidden Games\" option on the \"View\" menu.";

    hideRow.append(hideHeader, hideDescription);

    const privateRow = document.createElement("section");
    privateRow.className = "game-properties-privacy-row";

    const privateHeader = document.createElement("div");
    privateHeader.className = "game-properties-privacy-header";

    const privateTitle = document.createElement("p");
    privateTitle.className = "game-properties-privacy-title";
    privateTitle.textContent = "Mark as Private";

    const privateToggle = document.createElement("label");
    privateToggle.className = "game-properties-switch-control";

    const privateToggleInput = document.createElement("input");
    privateToggleInput.type = "checkbox";
    privateToggleInput.className = "game-properties-switch-input";
    privateToggleInput.checked = currentPrivacySettings.markAsPrivate;
    privateToggleInput.setAttribute("aria-label", "Mark as Private");

    const privateTrack = document.createElement("span");
    privateTrack.className = "game-properties-switch-track";
    privateTrack.setAttribute("aria-hidden", "true");

    privateToggle.append(privateToggleInput, privateTrack);
    privateHeader.append(privateTitle, privateToggle);

    const privateDescription = document.createElement("p");
    privateDescription.className = "game-properties-privacy-description";
    privateDescription.textContent = "Hide your activity in this game from others. Your In-Game status will not be visible to anyone else, the game will not show on your Steam Community profile, and any activity in the game will not show in friends' activity feeds.";

    privateRow.append(privateHeader, privateDescription);

    const overlayRow = document.createElement("section");
    overlayRow.className = "game-properties-privacy-row";

    const overlayHeader = document.createElement("div");
    overlayHeader.className = "game-properties-privacy-header";

    const overlayTitle = document.createElement("p");
    overlayTitle.className = "game-properties-privacy-title";
    overlayTitle.textContent = "In-Game Overlay Data";

    const overlayDeleteButton = document.createElement("button");
    overlayDeleteButton.type = "button";
    overlayDeleteButton.className = "game-properties-installed-action";
    overlayDeleteButton.textContent = currentPrivacySettings.overlayDataDeleted ? "Deleted" : "Delete";
    overlayDeleteButton.disabled = currentPrivacySettings.overlayDataDeleted;

    overlayHeader.append(overlayTitle, overlayDeleteButton);

    const overlayDescription = document.createElement("p");
    overlayDescription.className = "game-properties-privacy-description";
    overlayDescription.textContent = "Data that is used by the in-game overlay for this game, such as what browser tabs are open, windows that have been opened or pinned, and sort order for the game overview.";

    const overlayStatus = document.createElement("p");
    overlayStatus.className = "game-properties-privacy-status";
    overlayStatus.textContent = "Overlay data deleted.";
    overlayStatus.hidden = !currentPrivacySettings.overlayDataDeleted;

    const privacyFeedback = document.createElement("p");
    privacyFeedback.className = "game-properties-privacy-feedback";

    const applyPrivacyStatus = (status: PrivacyStatus): void => {
      currentPrivacyStatus = status;
      privacyFeedback.hidden = status.message.trim().length === 0;
      privacyFeedback.textContent = status.message;
      privacyFeedback.classList.toggle("is-success", status.kind === "success");
      privacyFeedback.classList.toggle("is-error", status.kind === "error");
    };

    applyPrivacyStatus(currentPrivacyStatus);
    overlayRow.append(overlayHeader, overlayDescription, overlayStatus);
    section.append(hideRow, privateRow, overlayRow);
    tabPanel.append(heading, section, privacyFeedback);

    const setToggleBusyState = (isBusy: boolean): void => {
      hideToggleInput.disabled = isBusy;
      privateToggleInput.disabled = isBusy;
      overlayDeleteButton.disabled = isBusy || currentPrivacySettings.overlayDataDeleted;
    };

    const syncPrivacySettings = async (restore: () => void): Promise<void> => {
      if (!currentSetPrivacySettings) {
        applyPrivacyStatus({
          kind: "idle",
          message: "",
        });
        return;
      }

      setToggleBusyState(true);
      try {
        await currentSetPrivacySettings({
          hideInLibrary: currentPrivacySettings.hideInLibrary,
          markAsPrivate: currentPrivacySettings.markAsPrivate,
        });
        applyPrivacyStatus({
          kind: "success",
          message: "Privacy settings updated.",
        });
      } catch {
        restore();
        persistCurrentSettings();
        applyPrivacyStatus({
          kind: "error",
          message: "Could not update privacy settings right now.",
        });
      } finally {
        setToggleBusyState(false);
      }
    };

    hideToggleInput.addEventListener("change", () => {
      const previousValue = currentPrivacySettings.hideInLibrary;
      currentPrivacySettings = {
        ...currentPrivacySettings,
        hideInLibrary: hideToggleInput.checked,
      };
      persistCurrentSettings();
      void syncPrivacySettings(() => {
        currentPrivacySettings = {
          ...currentPrivacySettings,
          hideInLibrary: previousValue,
        };
        hideToggleInput.checked = previousValue;
      });
    });

    privateToggleInput.addEventListener("change", () => {
      const previousValue = currentPrivacySettings.markAsPrivate;
      currentPrivacySettings = {
        ...currentPrivacySettings,
        markAsPrivate: privateToggleInput.checked,
      };
      persistCurrentSettings();
      void syncPrivacySettings(() => {
        currentPrivacySettings = {
          ...currentPrivacySettings,
          markAsPrivate: previousValue,
        };
        privateToggleInput.checked = previousValue;
      });
    });

    overlayDeleteButton.addEventListener("click", async () => {
      if (currentPrivacySettings.overlayDataDeleted) {
        return;
      }

      const previousValue = currentPrivacySettings.overlayDataDeleted;
      currentPrivacySettings = {
        ...currentPrivacySettings,
        overlayDataDeleted: true,
      };
      overlayDeleteButton.textContent = "Deleted";
      overlayDeleteButton.disabled = true;
      overlayStatus.hidden = false;
      persistCurrentSettings();

      if (!currentDeleteOverlayData) {
        applyPrivacyStatus({
          kind: "success",
          message: "Overlay data deleted.",
        });
        return;
      }

      setToggleBusyState(true);
      try {
        await currentDeleteOverlayData();
        applyPrivacyStatus({
          kind: "success",
          message: "Overlay data deleted.",
        });
      } catch {
        currentPrivacySettings = {
          ...currentPrivacySettings,
          overlayDataDeleted: previousValue,
        };
        overlayDeleteButton.textContent = "Delete";
        overlayDeleteButton.disabled = false;
        overlayStatus.hidden = true;
        persistCurrentSettings();
        applyPrivacyStatus({
          kind: "error",
          message: "Could not delete overlay data right now.",
        });
      } finally {
        setToggleBusyState(false);
      }
    });
  };

  const renderPlaceholderTab = (tabId: GamePropertiesTabId): void => {
    const heading = document.createElement("h3");
    heading.className = "game-properties-section-title";
    heading.textContent = getTabLabel(tabId);

    const placeholder = document.createElement("p");
    placeholder.className = "game-properties-placeholder";
    placeholder.textContent = "Coming soon.";

    tabPanel.append(heading, placeholder);
  };

  function renderTabContent(): void {
    cleanupRenderCallbacks();
    tabPanel.replaceChildren();

    for (const tab of GAME_PROPERTIES_TABS) {
      const button = tabButtons.get(tab.id);
      if (!button) {
        continue;
      }

      const isActive = tab.id === currentTab;
      button.classList.toggle("is-active", isActive);
      button.setAttribute("aria-selected", isActive ? "true" : "false");
      button.tabIndex = isActive ? 0 : -1;
    }

    const activeTabButton = tabButtons.get(currentTab);
    if (activeTabButton) {
      tabPanel.setAttribute("aria-labelledby", activeTabButton.id);
    }

    if (currentTab === "general") {
      renderGeneralTab();
      return;
    }

    if (currentTab === "compatibility") {
      renderCompatibilityTab();
      return;
    }

    if (currentTab === "updates") {
      renderUpdatesTab();
      return;
    }

    if (currentTab === "installed-files") {
      renderInstalledFilesTab();
      return;
    }

    if (currentTab === "game-versions-betas") {
      renderGameVersionsBetasTab();
      return;
    }

    if (currentTab === "controller") {
      renderControllerTab();
      return;
    }

    if (currentTab === "privacy") {
      renderPrivacyTab();
      return;
    }

    renderPlaceholderTab(currentTab);
  }

  for (const tab of GAME_PROPERTIES_TABS) {
    const button = document.createElement("button");
    button.type = "button";
    button.id = `game-properties-tab-${tab.id}`;
    button.className = "game-properties-nav-item";
    button.setAttribute("role", "tab");
    button.setAttribute("aria-controls", tabPanel.id);
    button.textContent = tab.label;

    button.addEventListener("click", () => {
      setTab(tab.id);
    });

    button.addEventListener("keydown", (event) => {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        moveTabSelection(tab.id, 1);
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        moveTabSelection(tab.id, -1);
        return;
      }

      if (event.key === "Home") {
        event.preventDefault();
        const firstTabId = GAME_PROPERTIES_TABS[0].id;
        setTab(firstTabId);
        tabButtons.get(firstTabId)?.focus();
        return;
      }

      if (event.key === "End") {
        event.preventDefault();
        const lastTabId = GAME_PROPERTIES_TABS[GAME_PROPERTIES_TABS.length - 1].id;
        setTab(lastTabId);
        tabButtons.get(lastTabId)?.focus();
      }
    });

    tabButtons.set(tab.id, button);
    tabList.append(button);
  }

  const close = (): void => {
    if (backdrop.hidden) {
      return;
    }

    cleanupRenderCallbacks();
    backdrop.hidden = true;
    currentGame = null;
    currentGameId = null;
    currentAvailableLanguages = [...DEFAULT_LANGUAGE_OPTIONS];
    currentAvailableVersionOptions = cloneGameVersionOptions(DEFAULT_GAME_VERSION_BETA_OPTIONS);
    currentAvailableVersionOptionsWarning = "";
    currentInstallationDetails = null;
    currentGeneralSettings = cloneGeneralSettings(DEFAULT_GENERAL_SETTINGS);
    currentCompatibilitySettings = cloneCompatibilitySettings(DEFAULT_COMPATIBILITY_SETTINGS);
    currentControllerSettings = cloneControllerSettings(DEFAULT_CONTROLLER_SETTINGS);
    currentPrivacySettings = clonePrivacySettings(DEFAULT_PRIVACY_SETTINGS);
    currentGameVersionsBetasSettings = cloneGameVersionsBetasSettings(DEFAULT_GAME_VERSIONS_BETAS_SETTINGS);
    currentUpdatesSettings = cloneUpdatesSettings(DEFAULT_UPDATES_SETTINGS);
    currentSaveSettings = null;
    currentBrowseInstalledFiles = null;
    currentBackupInstalledFiles = null;
    currentVerifyInstalledFiles = null;
    currentInstalledFilesStatus = {
      kind: "idle",
      message: "",
    };
    currentSetPrivacySettings = null;
    currentDeleteOverlayData = null;
    currentPrivacyStatus = {
      kind: "idle",
      message: "",
    };
    currentValidateBetaAccessCode = null;
    currentBetaValidationStatus = {
      kind: "idle",
      message: "",
    };
    if (persistDebounceTimeoutId !== null) {
      window.clearTimeout(persistDebounceTimeoutId);
      persistDebounceTimeoutId = null;
    }
    persistRequestSequence = 0;

    if (lastFocusedElement && document.contains(lastFocusedElement)) {
      lastFocusedElement.focus();
    }
  };

  const open = (input: GamePropertiesInput): void => {
    if (backdrop.hidden && document.activeElement instanceof HTMLElement) {
      lastFocusedElement = document.activeElement;
    }
    if (persistDebounceTimeoutId !== null) {
      window.clearTimeout(persistDebounceTimeoutId);
      persistDebounceTimeoutId = null;
    }
    persistRequestSequence = 0;

    currentGame = input.game;
    currentGameId = input.game.id;
    currentAvailableLanguages = normalizeLanguageOptions(input.availableLanguages ?? []);
    currentSaveSettings = input.saveSettings ?? null;
    currentInstallationDetails = input.installationDetails
      ? {
        installPath: input.installationDetails.installPath?.trim(),
        sizeOnDiskBytes: typeof input.installationDetails.sizeOnDiskBytes === "number"
          ? input.installationDetails.sizeOnDiskBytes
          : undefined,
      }
      : null;
    currentBrowseInstalledFiles = input.browseInstalledFiles ?? null;
    currentBackupInstalledFiles = input.backupInstalledFiles ?? null;
    currentVerifyInstalledFiles = input.verifyInstalledFiles ?? null;
    currentSetPrivacySettings = input.setPrivacySettings ?? null;
    currentDeleteOverlayData = input.deleteOverlayData ?? null;
    currentValidateBetaAccessCode = input.validateBetaAccessCode ?? null;
    const persistedSettings = parseGamePropertiesPersistedSettings(input.persistedSettings);
    currentGeneralSettings = cloneGeneralSettings(persistedSettings.general);
    currentCompatibilitySettings = cloneCompatibilitySettings(persistedSettings.compatibility);
    currentControllerSettings = cloneControllerSettings(persistedSettings.controller);
    currentPrivacySettings = input.privacySettings
      ? clonePrivacySettings(input.privacySettings)
      : clonePrivacySettings(DEFAULT_PRIVACY_SETTINGS);
    currentGameVersionsBetasSettings = cloneGameVersionsBetasSettings(persistedSettings.gameVersionsBetas);
    currentAvailableVersionOptions = resolveGameVersionBetaOptions(
      input.availableVersionOptions ?? DEFAULT_GAME_VERSION_BETA_OPTIONS,
      currentGameVersionsBetasSettings.selectedVersionId
    );
    currentAvailableVersionOptionsWarning = input.availableVersionOptionsWarning?.trim() ?? "";
    currentBetaValidationStatus = {
      kind: "idle",
      message: "",
    };
    currentInstalledFilesStatus = {
      kind: "idle",
      message: "",
    };
    currentPrivacyStatus = {
      kind: "idle",
      message: "",
    };
    currentUpdatesSettings = cloneUpdatesSettings(persistedSettings.updates);
    currentTab = "general";

    title.textContent = `${input.game.name} Properties`;
    subtitle.textContent = "Manage launch and in-game behavior.";
    renderTabContent();

    backdrop.hidden = false;
    tabButtons.get(currentTab)?.focus();
  };

  closeButton.addEventListener("click", close);

  backdrop.addEventListener("pointerdown", (event) => {
    if (event.target === backdrop) {
      close();
    }
  });

  window.addEventListener("keydown", (event) => {
    if (backdrop.hidden) {
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      close();
      return;
    }

    if (event.key !== "Tab") {
      return;
    }

    const focusableElements = getFocusableElements(panel);
    if (focusableElements.length === 0) {
      event.preventDefault();
      return;
    }

    const firstFocusable = focusableElements[0];
    const lastFocusable = focusableElements[focusableElements.length - 1];
    const activeElement = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;

    if (event.shiftKey) {
      if (activeElement === firstFocusable || !activeElement || !panel.contains(activeElement)) {
        event.preventDefault();
        lastFocusable.focus();
      }
      return;
    }

    if (activeElement === lastFocusable || !activeElement || !panel.contains(activeElement)) {
      event.preventDefault();
      firstFocusable.focus();
    }
  });

  return {
    close,
    open,
  };
};
