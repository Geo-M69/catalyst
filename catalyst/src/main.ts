import { ipcService } from "./shared/ipc/client";
import { normalizeAppError } from "./shared/ipc/errors";
import type { AppErrorPayload } from "./shared/ipc/contracts";

export {};

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

const setStatusErrorMetadata = (appError: AppErrorPayload | null): void => {
  if (!appError) {
    delete statusMessageElement.dataset.errorKind;
    delete statusMessageElement.dataset.errorCode;
    return;
  }

  statusMessageElement.dataset.errorKind = appError.kind;
  statusMessageElement.dataset.errorCode = appError.code;
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

const refreshSession = async (): Promise<boolean> => {
  try {
    const user = await ipcService.getSession();

    if (!user) {
      setStatusErrorMetadata(null);
      setStatusMessage("Not logged in. Click below to sign in with Steam.");
      steamButtonElement.textContent = "Login with Steam";
      return false;
    }

    window.location.replace(MAIN_PAGE_PATH);
    return true;
  } catch (error) {
    const appError = normalizeAppError(error, "Could not read current app session.");
    setStatusErrorMetadata(appError);
    setStatusMessage(appError.message, true);
    console.error(`[auth/session] ${appError.kind}:${appError.code} ${appError.message}`);
    steamButtonElement.textContent = "Login with Steam";
    return false;
  }
};

const startSteamLogin = async (): Promise<void> => {
  try {
    setPendingState(true);

    const result = await ipcService.startSteamAuth();
    const steamId = result.user.steamId ?? "unknown";
    window.history.replaceState(
      {},
      "",
      `${window.location.pathname}?status=success&steamId=${encodeURIComponent(steamId)}&syncedGames=${result.syncedGames}`
    );
    window.location.replace(MAIN_PAGE_PATH);
  } catch (error) {
    const appError = normalizeAppError(error, "Could not start Steam login");
    setStatusErrorMetadata(appError);
    setStatusMessage(appError.message, true);
    console.error(`[auth/start_steam_auth] ${appError.kind}:${appError.code} ${appError.message}`);
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

  // Unhide the document now that we've confirmed the user is not redirected
  try {
    document.documentElement.style.visibility = "visible";
  } catch {
    // ignore if DOM access fails for some reason
  }

  revealAuthPanel();
};

void initialize();
