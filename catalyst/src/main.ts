import { invoke } from "@tauri-apps/api/core";

export {};

interface PublicUser {
  id: string;
  email: string;
  steamLinked: boolean;
  steamId?: string;
}

interface SteamAuthResponse {
  user: PublicUser;
  syncedGames: number;
}

const SPLASH_DURATION_MS = 3000;
const TITLE_FADE_OUT_MS = 450;
const MAIN_PAGE_PATH = "/main.html";

const welcomeTitleElement = document.getElementById("welcome-title");
const authPanelElement = document.getElementById("auth-panel");
const statusMessageElement = document.getElementById("status-message");
const steamButtonElement = document.getElementById("steam-login");

if (
  !(welcomeTitleElement instanceof HTMLElement) ||
  !(authPanelElement instanceof HTMLElement) ||
  !(statusMessageElement instanceof HTMLElement) ||
  !(steamButtonElement instanceof HTMLButtonElement)
) {
  throw new Error("App root is missing required DOM elements");
}

const setStatusMessage = (message: string, isError = false): void => {
  statusMessageElement.textContent = message;
  statusMessageElement.classList.toggle("status-error", isError);
  if (isError) {
    statusMessageElement.classList.remove("sr-only");
  }
};

const setPendingState = (isPending: boolean): void => {
  steamButtonElement.disabled = isPending;
};

const replayFadeInInReverse = (element: HTMLElement, durationMs: number): void => {
  element.style.setProperty("--title-fade-direction", "reverse");
  element.style.setProperty("--title-fade-duration", `${durationMs}ms`);
  element.classList.remove("fade-in");
  void element.offsetWidth;
  element.classList.add("fade-in");
};

const revealAuthPanel = (): void => {
  const fadeOutStartMs = Math.max(0, SPLASH_DURATION_MS - TITLE_FADE_OUT_MS);

  window.setTimeout(() => {
    replayFadeInInReverse(welcomeTitleElement, TITLE_FADE_OUT_MS);
  }, fadeOutStartMs);

  window.setTimeout(() => {
    welcomeTitleElement.hidden = true;
    authPanelElement.hidden = false;
    authPanelElement.classList.add("fade-in");
  }, SPLASH_DURATION_MS);
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

const refreshSession = async (): Promise<boolean> => {
  try {
    const user = await invoke<PublicUser | null>("get_session");

    if (!user) {
      setStatusMessage("Not logged in. Click below to sign in with Steam.");
      steamButtonElement.textContent = "Login with Steam";
      return false;
    }

    window.location.replace(MAIN_PAGE_PATH);
    return true;
  } catch (error) {
    setStatusMessage(toErrorMessage(error, "Could not read current app session."), true);
    steamButtonElement.textContent = "Login with Steam";
    return false;
  }
};

const startSteamLogin = async (): Promise<void> => {
  try {
    setPendingState(true);

    const result = await invoke<SteamAuthResponse>("start_steam_auth");
    const steamId = result.user.steamId ?? "unknown";
    window.history.replaceState(
      {},
      "",
      `${window.location.pathname}?status=success&steamId=${encodeURIComponent(steamId)}&syncedGames=${result.syncedGames}`
    );
    window.location.replace(MAIN_PAGE_PATH);
  } catch (error) {
    setStatusMessage(toErrorMessage(error, "Could not start Steam login"), true);
    setPendingState(false);
  }
};

steamButtonElement.addEventListener("click", () => {
  void startSteamLogin();
});

const initialize = async (): Promise<void> => {
  const redirected = await refreshSession();
  if (redirected) {
    return;
  }

  revealAuthPanel();
};

void initialize();
