import { invoke } from "@tauri-apps/api/core";

export {};

interface PublicUser {
  id: string;
  email: string;
  steamLinked: boolean;
  steamId?: string;
}

interface SteamStatusResponse {
  userId: string;
  provider: string;
  linked: boolean;
  steamId?: string;
}

interface SteamSyncResponse {
  userId: string;
  provider: string;
  syncedGames: number;
}

interface GameResponse {
  id: string;
  provider: string;
  externalId: string;
  name: string;
  playtimeMinutes: number;
  artworkUrl?: string;
  lastSyncedAt: string;
}

interface LibraryResponse {
  userId: string;
  total: number;
  games: GameResponse[];
}

const statusElement = document.getElementById("main-status");
const steamStatusElement = document.getElementById("steam-status");
const librarySummaryElement = document.getElementById("library-summary");
const libraryListElement = document.getElementById("library-list");
const steamLinkButton = document.getElementById("steam-link-button");
const steamSyncButton = document.getElementById("steam-sync-button");
const refreshLibraryButton = document.getElementById("refresh-library-button");
const logoutButton = document.getElementById("logout-button");

if (
  !(statusElement instanceof HTMLElement) ||
  !(steamStatusElement instanceof HTMLElement) ||
  !(librarySummaryElement instanceof HTMLElement) ||
  !(libraryListElement instanceof HTMLUListElement) ||
  !(steamLinkButton instanceof HTMLButtonElement) ||
  !(steamSyncButton instanceof HTMLButtonElement) ||
  !(refreshLibraryButton instanceof HTMLButtonElement) ||
  !(logoutButton instanceof HTMLButtonElement)
) {
  throw new Error("Main page is missing required DOM elements");
}

let isPending = false;
let isSteamLinked = false;

const setStatus = (message: string, isError = false): void => {
  statusElement.textContent = message;
  statusElement.classList.toggle("status-error", isError);
};

const setSteamStatus = (message: string, isError = false): void => {
  steamStatusElement.textContent = message;
  steamStatusElement.classList.toggle("status-error", isError);
};

const setLibrarySummary = (message: string, isError = false): void => {
  librarySummaryElement.textContent = message;
  librarySummaryElement.classList.toggle("status-error", isError);
};

const applyControlState = (): void => {
  steamLinkButton.disabled = isPending;
  steamSyncButton.disabled = isPending || !isSteamLinked;
  refreshLibraryButton.disabled = isPending;
  logoutButton.disabled = isPending;
};

const setPendingState = (pending: boolean): void => {
  isPending = pending;
  applyControlState();
};

const toErrorMessage = (error: unknown, fallbackMessage: string): string => {
  if (typeof error === "string" && error.trim().length > 0) {
    return error;
  }

  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }

  return fallbackMessage;
};

const renderLibrary = (games: GameResponse[]): void => {
  libraryListElement.replaceChildren();

  if (games.length === 0) {
    const emptyItem = document.createElement("li");
    emptyItem.textContent = "No games synced yet.";
    libraryListElement.append(emptyItem);
    return;
  }

  for (const game of games) {
    const item = document.createElement("li");
    const hours = (game.playtimeMinutes / 60).toFixed(1);
    item.textContent = `${game.name} (${game.provider.toUpperCase()}) - ${hours}h`;
    libraryListElement.append(item);
  }
};

const refreshLibrary = async (): Promise<void> => {
  try {
    const library = await invoke<LibraryResponse>("get_library");
    renderLibrary(library.games);
    setLibrarySummary(`${library.total} games in your library.`);
  } catch (error) {
    setLibrarySummary(toErrorMessage(error, "Could not load library."), true);
  }
};

const refreshSteamStatus = async (): Promise<void> => {
  try {
    const status = await invoke<SteamStatusResponse>("get_steam_status");

    isSteamLinked = status.linked;
    applyControlState();
    steamLinkButton.textContent = status.linked ? "Reconnect Steam" : "Connect Steam";
    if (status.linked) {
      setSteamStatus(`Steam linked (${status.steamId ?? "unknown"}).`);
      return;
    }

    setSteamStatus("Steam is not linked to this account.");
  } catch (error) {
    isSteamLinked = false;
    applyControlState();
    setSteamStatus(toErrorMessage(error, "Could not load Steam status."), true);
  }
};

const refreshSession = async (): Promise<boolean> => {
  try {
    const user = await invoke<PublicUser | null>("get_session");
    if (!user) {
      window.location.replace("/");
      return false;
    }

    setStatus(`Signed in as ${user.email}.`);
    return true;
  } catch (error) {
    setStatus(toErrorMessage(error, "Could not load session data."), true);
    return false;
  }
};

const connectSteam = async (): Promise<void> => {
  try {
    setPendingState(true);
    setStatus(
      "Opening Steam login in your browser. If Windows Firewall prompts for Catalyst, allow local/private access."
    );
    const result = await invoke<{ user: PublicUser; syncedGames: number }>("start_steam_auth");
    const { user, syncedGames } = result;
    setStatus(`Steam connected for ${user.email}. Synced ${syncedGames} games.`);
    await refreshSteamStatus();
    await refreshLibrary();
  } catch (error) {
    setStatus(toErrorMessage(error, "Could not connect Steam."), true);
  } finally {
    setPendingState(false);
  }
};

const syncSteam = async (): Promise<void> => {
  try {
    setPendingState(true);
    const result = await invoke<SteamSyncResponse>("sync_steam_library");
    setStatus(`Synced ${result.syncedGames} Steam games.`);
    await refreshLibrary();
  } catch (error) {
    setStatus(toErrorMessage(error, "Could not sync Steam library."), true);
  } finally {
    setPendingState(false);
  }
};

const logout = async (): Promise<void> => {
  try {
    setPendingState(true);
    await invoke("logout");
    window.location.replace("/");
  } catch (error) {
    setStatus(toErrorMessage(error, "Could not log out."), true);
    setPendingState(false);
  }
};

steamLinkButton.addEventListener("click", () => {
  void connectSteam();
});

steamSyncButton.addEventListener("click", () => {
  void syncSteam();
});

refreshLibraryButton.addEventListener("click", () => {
  void refreshLibrary();
});

logoutButton.addEventListener("click", () => {
  void logout();
});

const initialize = async (): Promise<void> => {
  applyControlState();
  const hasSession = await refreshSession();
  if (!hasSession) {
    return;
  }

  await refreshSteamStatus();
  await refreshLibrary();
};

void initialize();
