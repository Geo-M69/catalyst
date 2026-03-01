import type { GameResponse } from "../types";

export interface InstallDialogLocation {
  path: string;
  freeSpaceBytes?: number;
}

export interface InstallDialogInput {
  game: GameResponse;
  locations: InstallDialogLocation[];
  installSizeBytes?: number;
}

export interface InstallDialogResult {
  createDesktopShortcut: boolean;
  createApplicationShortcut: boolean;
  installPath: string;
}

export interface InstallDialogController {
  close: () => void;
  open: (input: InstallDialogInput) => Promise<InstallDialogResult | null>;
}

const DEFAULT_INSTALL_PATH = "Steam default install location";
const DEFAULT_INSTALL_SIZE_LABEL = "Size unavailable";
const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"];

const formatBytes = (sizeInBytes: number): string => {
  if (!Number.isFinite(sizeInBytes) || sizeInBytes <= 0) {
    return "Unknown";
  }

  let unitIndex = 0;
  let value = sizeInBytes;
  while (value >= 1024 && unitIndex < BYTE_UNITS.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const fractionDigits = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(fractionDigits)} ${BYTE_UNITS[unitIndex]}`;
};

const formatFreeSpaceLabel = (location: InstallDialogLocation): string => {
  const freeSpaceBytes = location.freeSpaceBytes;
  if (typeof freeSpaceBytes === "number" && Number.isFinite(freeSpaceBytes) && freeSpaceBytes > 0) {
    return `${formatBytes(freeSpaceBytes)} FREE`;
  }

  return "FREE SPACE UNKNOWN";
};

const initialsFromName = (name: string): string => {
  const words = name.trim().split(/\s+/).filter((part) => part.length > 0);
  return words.slice(0, 2).map((part) => part[0]?.toUpperCase() ?? "").join("") || "?";
};

const getArtworkCandidates = (game: GameResponse): string[] => {
  const candidates: string[] = [];
  const seen = new Set<string>();

  const pushCandidate = (value: string | undefined): void => {
    const trimmed = value?.trim();
    if (!trimmed || seen.has(trimmed)) {
      return;
    }
    seen.add(trimmed);
    candidates.push(trimmed);
  };

  if (game.provider.toLowerCase() === "steam" && /^\d+$/.test(game.externalId)) {
    const appId = game.externalId;
    pushCandidate(`https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/capsule_616x353.jpg`);
    pushCandidate(`https://cdn.cloudflare.steamstatic.com/steam/apps/${appId}/header.jpg`);
  }

  pushCandidate(game.artworkUrl);
  return candidates;
};

const dedupeLocations = (locations: InstallDialogLocation[]): InstallDialogLocation[] => {
  const dedupedLocations: InstallDialogLocation[] = [];
  const seenPaths = new Set<string>();

  for (const location of locations) {
    const normalizedPath = location.path.trim();
    if (normalizedPath.length === 0) {
      continue;
    }

    const pathKey = normalizedPath.toLocaleLowerCase();
    if (seenPaths.has(pathKey)) {
      continue;
    }

    seenPaths.add(pathKey);
    dedupedLocations.push({
      path: normalizedPath,
      freeSpaceBytes: location.freeSpaceBytes,
    });
  }

  if (dedupedLocations.length > 0) {
    return dedupedLocations;
  }

  return [{ path: DEFAULT_INSTALL_PATH }];
};

const findFocusableLocationOption = (optionButtons: HTMLButtonElement[]): HTMLButtonElement | null => {
  for (const optionButton of optionButtons) {
    if (optionButton.classList.contains("is-selected")) {
      return optionButton;
    }
  }

  return optionButtons[0] ?? null;
};

export const createInstallDialog = (): InstallDialogController => {
  const backdrop = document.createElement("div");
  backdrop.className = "install-dialog-backdrop";
  backdrop.hidden = true;

  const panel = document.createElement("section");
  panel.className = "install-dialog-panel";
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-modal", "true");
  panel.setAttribute("aria-labelledby", "install-dialog-title");

  const header = document.createElement("header");
  header.className = "install-dialog-header";

  const title = document.createElement("h3");
  title.id = "install-dialog-title";
  title.className = "install-dialog-title";
  title.textContent = "Install";

  const closeButton = document.createElement("button");
  closeButton.type = "button";
  closeButton.className = "install-dialog-close";
  closeButton.setAttribute("aria-label", "Close install dialog");
  closeButton.textContent = "Close";

  header.append(title, closeButton);

  const gameRow = document.createElement("div");
  gameRow.className = "install-dialog-game-row";

  const artworkShell = document.createElement("div");
  artworkShell.className = "install-dialog-artwork";

  const gameMeta = document.createElement("div");
  gameMeta.className = "install-dialog-game-meta";

  const gameName = document.createElement("p");
  gameName.className = "install-dialog-game-name";

  const gameProvider = document.createElement("p");
  gameProvider.className = "install-dialog-game-provider";

  const gameSize = document.createElement("p");
  gameSize.className = "install-dialog-game-size";

  gameMeta.append(gameName, gameProvider);
  gameRow.append(artworkShell, gameMeta, gameSize);

  const options = document.createElement("div");
  options.className = "install-dialog-options";

  const desktopShortcutLabel = document.createElement("label");
  desktopShortcutLabel.className = "install-dialog-checkbox";

  const desktopShortcutInput = document.createElement("input");
  desktopShortcutInput.type = "checkbox";
  desktopShortcutInput.className = "install-dialog-checkbox-input";
  desktopShortcutInput.checked = true;

  const desktopShortcutIndicator = document.createElement("span");
  desktopShortcutIndicator.className = "install-dialog-checkbox-indicator";
  desktopShortcutIndicator.setAttribute("aria-hidden", "true");

  const desktopShortcutText = document.createElement("span");
  desktopShortcutText.className = "install-dialog-checkbox-text";
  desktopShortcutText.textContent = "Create desktop shortcut";

  desktopShortcutLabel.append(desktopShortcutInput, desktopShortcutIndicator, desktopShortcutText);

  const applicationShortcutLabel = document.createElement("label");
  applicationShortcutLabel.className = "install-dialog-checkbox";

  const applicationShortcutInput = document.createElement("input");
  applicationShortcutInput.type = "checkbox";
  applicationShortcutInput.className = "install-dialog-checkbox-input";
  applicationShortcutInput.checked = true;

  const applicationShortcutIndicator = document.createElement("span");
  applicationShortcutIndicator.className = "install-dialog-checkbox-indicator";
  applicationShortcutIndicator.setAttribute("aria-hidden", "true");

  const applicationShortcutText = document.createElement("span");
  applicationShortcutText.className = "install-dialog-checkbox-text";
  applicationShortcutText.textContent = "Create an application shortcut";

  applicationShortcutLabel.append(applicationShortcutInput, applicationShortcutIndicator, applicationShortcutText);
  options.append(desktopShortcutLabel, applicationShortcutLabel);

  const locationSection = document.createElement("section");
  locationSection.className = "install-dialog-location-section";

  const locationTitle = document.createElement("p");
  locationTitle.className = "install-dialog-location-title";
  locationTitle.textContent = "Install To";

  const locationControls = document.createElement("div");
  locationControls.className = "install-dialog-location-controls";

  const locationSelect = document.createElement("div");
  locationSelect.className = "install-dialog-location-select";

  const locationTrigger = document.createElement("button");
  locationTrigger.type = "button";
  locationTrigger.className = "install-dialog-location-trigger";
  locationTrigger.setAttribute("aria-haspopup", "listbox");
  locationTrigger.setAttribute("aria-expanded", "false");
  locationTrigger.setAttribute("aria-controls", "install-dialog-location-menu");

  const locationTriggerSummary = document.createElement("span");
  locationTriggerSummary.className = "install-dialog-location-trigger-summary";

  const locationPathText = document.createElement("span");
  locationPathText.className = "install-dialog-location-path";

  const locationFreeText = document.createElement("span");
  locationFreeText.className = "install-dialog-location-free";

  const locationTriggerCaret = document.createElement("span");
  locationTriggerCaret.className = "install-dialog-location-caret";
  locationTriggerCaret.setAttribute("aria-hidden", "true");

  locationTriggerSummary.append(locationPathText, locationFreeText);
  locationTrigger.append(locationTriggerSummary, locationTriggerCaret);

  const locationSettingsButton = document.createElement("button");
  locationSettingsButton.type = "button";
  locationSettingsButton.className = "install-dialog-location-settings";
  locationSettingsButton.setAttribute("aria-label", "Steam install folder settings");
  locationSettingsButton.setAttribute("title", "Install location settings are managed by Steam.");
  locationSettingsButton.disabled = true;

  const locationSettingsIcon = document.createElement("span");
  locationSettingsIcon.className = "install-dialog-location-settings-icon";
  locationSettingsIcon.setAttribute("aria-hidden", "true");
  locationSettingsButton.append(locationSettingsIcon);

  locationControls.append(locationTrigger, locationSettingsButton);

  const locationMenu = document.createElement("div");
  locationMenu.id = "install-dialog-location-menu";
  locationMenu.className = "install-dialog-location-menu";
  locationMenu.setAttribute("role", "listbox");
  locationMenu.hidden = true;

  locationSelect.append(locationControls, locationMenu);
  locationSection.append(locationTitle, locationSelect);

  const actions = document.createElement("div");
  actions.className = "install-dialog-actions";

  const installButton = document.createElement("button");
  installButton.type = "button";
  installButton.className = "install-dialog-button install-dialog-button-install";
  installButton.textContent = "Install";

  const cancelButton = document.createElement("button");
  cancelButton.type = "button";
  cancelButton.className = "install-dialog-button install-dialog-button-cancel";
  cancelButton.textContent = "Cancel";

  actions.append(installButton, cancelButton);
  panel.append(header, gameRow, options, locationSection, actions);
  backdrop.append(panel);
  document.body.append(backdrop);

  let resolver: ((value: InstallDialogResult | null) => void) | null = null;
  let activeLocations: InstallDialogLocation[] = [{ path: DEFAULT_INSTALL_PATH }];
  let selectedInstallPath = DEFAULT_INSTALL_PATH;
  let locationOptionButtons: HTMLButtonElement[] = [];
  let previouslyFocusedElement: HTMLElement | null = null;

  const closeLocationMenu = (): void => {
    locationMenu.hidden = true;
    locationSelect.classList.remove("is-open");
    locationTrigger.setAttribute("aria-expanded", "false");
  };

  const openLocationMenu = (): void => {
    locationMenu.hidden = false;
    locationSelect.classList.add("is-open");
    locationTrigger.setAttribute("aria-expanded", "true");
  };

  const findLocationByPath = (path: string): InstallDialogLocation | null => {
    for (const location of activeLocations) {
      if (location.path === path) {
        return location;
      }
    }
    return null;
  };

  const updateLocationTrigger = (): void => {
    const selectedLocation = findLocationByPath(selectedInstallPath) ?? activeLocations[0];
    selectedInstallPath = selectedLocation.path;
    locationPathText.textContent = selectedLocation.path;
    locationFreeText.textContent = formatFreeSpaceLabel(selectedLocation);

    for (const optionButton of locationOptionButtons) {
      const optionPath = optionButton.dataset.installPath ?? "";
      const isSelected = optionPath === selectedInstallPath;
      optionButton.classList.toggle("is-selected", isSelected);
      optionButton.setAttribute("aria-selected", `${isSelected}`);
    }
  };

  const setSelectedInstallPath = (path: string): void => {
    if (!findLocationByPath(path)) {
      return;
    }

    selectedInstallPath = path;
    updateLocationTrigger();
  };

  const renderLocationOptions = (): void => {
    locationMenu.replaceChildren();
    locationOptionButtons = [];

    for (const location of activeLocations) {
      const optionButton = document.createElement("button");
      optionButton.type = "button";
      optionButton.className = "install-dialog-location-option";
      optionButton.dataset.installPath = location.path;
      optionButton.setAttribute("role", "option");
      optionButton.setAttribute("aria-selected", "false");

      const optionPath = document.createElement("span");
      optionPath.className = "install-dialog-location-path";
      optionPath.textContent = location.path;

      const optionFree = document.createElement("span");
      optionFree.className = "install-dialog-location-free";
      optionFree.textContent = formatFreeSpaceLabel(location);

      optionButton.append(optionPath, optionFree);
      optionButton.addEventListener("click", () => {
        setSelectedInstallPath(location.path);
        closeLocationMenu();
        locationTrigger.focus();
      });

      locationOptionButtons.push(optionButton);
      locationMenu.append(optionButton);
    }

    updateLocationTrigger();
  };

  const setArtwork = (game: GameResponse): void => {
    artworkShell.replaceChildren();

    const candidates = getArtworkCandidates(game);
    if (candidates.length === 0) {
      const placeholder = document.createElement("div");
      placeholder.className = "install-dialog-artwork-placeholder";
      placeholder.textContent = initialsFromName(game.name);
      artworkShell.append(placeholder);
      return;
    }

    const image = document.createElement("img");
    image.className = "install-dialog-artwork-image";
    image.alt = `${game.name} artwork`;
    let candidateIndex = 0;

    image.addEventListener("error", () => {
      candidateIndex += 1;
      if (candidateIndex < candidates.length) {
        image.src = candidates[candidateIndex];
        return;
      }

      image.remove();
      const placeholder = document.createElement("div");
      placeholder.className = "install-dialog-artwork-placeholder";
      placeholder.textContent = initialsFromName(game.name);
      artworkShell.append(placeholder);
    });

    image.src = candidates[candidateIndex];
    artworkShell.append(image);
  };

  const finish = (value: InstallDialogResult | null): void => {
    closeLocationMenu();
    backdrop.hidden = true;

    const currentResolver = resolver;
    resolver = null;
    if (currentResolver) {
      currentResolver(value);
    }

    previouslyFocusedElement?.focus();
    previouslyFocusedElement = null;
  };

  const close = (): void => {
    if (backdrop.hidden) {
      return;
    }

    finish(null);
  };

  const submit = (): void => {
    const selectedLocation = findLocationByPath(selectedInstallPath);
    if (!selectedLocation) {
      return;
    }

    finish({
      createDesktopShortcut: desktopShortcutInput.checked,
      createApplicationShortcut: applicationShortcutInput.checked,
      installPath: selectedLocation.path,
    });
  };

  closeButton.addEventListener("click", close);
  cancelButton.addEventListener("click", close);
  installButton.addEventListener("click", submit);

  locationTrigger.addEventListener("click", () => {
    if (locationMenu.hidden) {
      openLocationMenu();
      findFocusableLocationOption(locationOptionButtons)?.focus();
      return;
    }

    closeLocationMenu();
  });

  locationTrigger.addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      openLocationMenu();
      findFocusableLocationOption(locationOptionButtons)?.focus();
    }
  });

  locationMenu.addEventListener("keydown", (event) => {
    if (locationOptionButtons.length === 0) {
      return;
    }

    const activeOption = document.activeElement instanceof HTMLButtonElement ? document.activeElement : null;
    const currentIndex = activeOption ? locationOptionButtons.indexOf(activeOption) : -1;

    if (event.key === "Escape") {
      event.preventDefault();
      closeLocationMenu();
      locationTrigger.focus();
      return;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      const nextIndex = currentIndex < 0 ? 0 : (currentIndex + 1) % locationOptionButtons.length;
      locationOptionButtons[nextIndex].focus();
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      const previousIndex = currentIndex < 0
        ? locationOptionButtons.length - 1
        : (currentIndex - 1 + locationOptionButtons.length) % locationOptionButtons.length;
      locationOptionButtons[previousIndex].focus();
      return;
    }

    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      activeOption?.click();
    }
  });

  backdrop.addEventListener("pointerdown", (event) => {
    if (event.target === backdrop) {
      close();
    }
  });

  document.addEventListener("pointerdown", (event) => {
    if (backdrop.hidden || locationMenu.hidden) {
      return;
    }

    const target = event.target;
    if (target instanceof Node && locationSelect.contains(target)) {
      return;
    }

    closeLocationMenu();
  });

  window.addEventListener("keydown", (event) => {
    if (backdrop.hidden || event.key !== "Escape") {
      return;
    }

    event.preventDefault();
    if (!locationMenu.hidden) {
      closeLocationMenu();
      locationTrigger.focus();
      return;
    }

    close();
  });

  return {
    close,
    open: ({ game, locations, installSizeBytes }: InstallDialogInput) => {
      if (resolver) {
        finish(null);
      }

      previouslyFocusedElement = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      activeLocations = dedupeLocations(locations);
      selectedInstallPath = activeLocations[0].path;
      desktopShortcutInput.checked = true;
      applicationShortcutInput.checked = true;

      setArtwork(game);
      gameName.textContent = game.name;
      gameProvider.textContent = game.provider.toUpperCase();
      gameSize.textContent = typeof installSizeBytes === "number"
        && Number.isFinite(installSizeBytes)
        && installSizeBytes > 0
        ? formatBytes(installSizeBytes)
        : DEFAULT_INSTALL_SIZE_LABEL;

      renderLocationOptions();
      backdrop.hidden = false;
      closeLocationMenu();
      installButton.focus();

      return new Promise((resolve) => {
        resolver = resolve;
      });
    },
  };
};
