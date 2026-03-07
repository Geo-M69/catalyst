/// <reference types="vite/client" />
import { ipcService } from "./shared/ipc/client";
import { normalizeAppError } from "./shared/ipc/errors";
import type { AppErrorPayload } from "./shared/ipc/contracts";
import { listen } from "@tauri-apps/api/event";

// Inject a relaxed Content-Security-Policy during development to avoid
// blocking inline styles or dev-only assets. This keeps the stricter
// production CSP in `src-tauri/tauri.conf.json` while allowing a smoother
// developer experience when running Vite locally.
if (import.meta.env.DEV) {
  try {
    const meta = document.createElement("meta");
    meta.setAttribute("http-equiv", "Content-Security-Policy");
    meta.setAttribute(
      "content",
      "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' data: https://fonts.gstatic.com; img-src 'self' data: blob: https:; connect-src 'self' http://localhost:1420 ws://localhost:1421 https://api.steampowered.com https://store.steampowered.com; object-src 'none'; base-uri 'self'; frame-ancestors 'none';"
    );
    document.head.prepend(meta);
  } catch (e) {
    // Fail gracefully in environments where `document.head` isn't available.
    // This is purely a developer convenience and must not affect production.
    // eslint-disable-next-line no-console
    console.warn("Failed to inject dev CSP meta tag:", e);
  }
}

export {};

const SPLASH_DURATION_MS = 3000;
const TITLE_FADE_OUT_MS = 450;
const MAIN_PAGE_PATH = "#/main";

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

    // Navigate to the main view within the single-page index by setting the hash
    // and dynamically importing the main page module.
    window.location.hash = MAIN_PAGE_PATH;
    try {
      await import("./mainPage/mainPage");
      // Unhide merged main page markup
      const mainRoot = document.getElementById("library-root");
      if (mainRoot instanceof HTMLElement) {
        mainRoot.hidden = false;
      }
    } catch (e) {
      console.error("Failed to initialize main page:", e);
    }
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
    window.history.replaceState({}, "", `${window.location.pathname}?status=success&steamId=${encodeURIComponent(steamId)}&syncedGames=${result.syncedGames}`);
    // Switch to main view without a full page reload
    window.location.hash = MAIN_PAGE_PATH;
    try {
      await import("./mainPage/mainPage");
      const mainRoot = document.getElementById("library-root");
      if (mainRoot instanceof HTMLElement) {
        mainRoot.hidden = false;
      }
    } catch (e) {
      console.error("Failed to initialize main page after auth:", e);
    }
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

// If the URL points to the main view on load (e.g., deep link), initialize it.
if (window.location.hash === MAIN_PAGE_PATH) {
  void (async () => {
    try {
      await import("./mainPage/mainPage");
      const mainRoot = document.getElementById("library-root");
      if (mainRoot instanceof HTMLElement) {
        mainRoot.hidden = false;
      }
    } catch (e) {
      console.error("Failed to initialize main page from hash:", e);
    }
  })();
}

// Frontend listener for background local Steam scan results
// Emits: `local-scan-complete` (payload: number[]), `local-scan-error` (payload: string)
void (async () => {
  try {
    await listen<number[]>("local-scan-complete", event => {
      console.info("Local Steam scan complete:", event.payload);
      // Example: store into localCache or trigger UI refresh
    });

    await listen<string>("local-scan-error", event => {
      console.error("Local Steam scan error:", event.payload);
      // Example: show toast or status message
      setStatusMessage(`Local scan failed: ${event.payload}` , true);
    });
  } catch (err) {
    console.warn("Could not register local-scan listeners (not running in Tauri?):", err);
  }
})();
