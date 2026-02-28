export {};

interface PublicUser {
  id: string;
  email: string;
  steamLinked: boolean;
  steamId?: string;
}

interface SessionResponse {
  user: PublicUser;
}

interface SteamStartResponse {
  authorizationUrl: string;
}

const BACKEND_BASE_URL = "http://localhost:4000";
const SPLASH_DURATION_MS = 3000;
const TITLE_FADE_OUT_MS = 450;

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
    authPanelElement.classList.add("auth-card-reveal");
  }, SPLASH_DURATION_MS);
};

const parseErrorMessage = async (response: Response): Promise<string> => {
  try {
    const body = (await response.json()) as {
      error?: {
        message?: string;
      };
    };

    return body.error?.message ?? `Request failed with status ${response.status}`;
  } catch {
    return `Request failed with status ${response.status}`;
  }
};

const requestJson = async <T>(path: string): Promise<T> => {
  let response: Response;

  try {
    response = await fetch(`${BACKEND_BASE_URL}${path}`, {
      method: "GET",
      credentials: "include"
    });
  } catch {
    throw new Error(
      `Could not reach backend at ${BACKEND_BASE_URL}. Start backend and confirm CORS/origin settings.`
    );
  }

  if (!response.ok) {
    throw new Error(await parseErrorMessage(response));
  }

  return (await response.json()) as T;
};

const refreshSession = async (): Promise<void> => {
  try {
    const session = await requestJson<SessionResponse>("/auth/session");

    if (session.user.steamLinked) {
      setStatusMessage(`Logged in with Steam (${session.user.steamId ?? "unknown"}).`);
      steamButtonElement.textContent = "Reconnect Steam";
      return;
    }

    setStatusMessage("Account session exists but Steam is not linked yet.");
    steamButtonElement.textContent = "Connect Steam";
  } catch {
    setStatusMessage("Not logged in. Click below to sign in with Steam.");
    steamButtonElement.textContent = "Login with Steam";
  }
};

const applySteamCallbackStatusFromQuery = (): void => {
  const params = new URLSearchParams(window.location.search);
  const status = params.get("status");
  if (!status) {
    return;
  }

  if (status === "success") {
    window.location.replace("/src/mainPage/mainPage.html");
    return;
  } else {
    const message = params.get("message") ?? "Steam login failed.";
    setStatusMessage(message, true);
  }

  params.delete("status");
  params.delete("userId");
  params.delete("steamId");
  params.delete("syncedGames");
  params.delete("message");

  const nextQuery = params.toString();
  const nextUrl = `${window.location.pathname}${nextQuery.length > 0 ? `?${nextQuery}` : ""}`;
  window.history.replaceState({}, "", nextUrl);
};

const startSteamLogin = async (): Promise<void> => {
  try {
    setPendingState(true);

    const result = await requestJson<SteamStartResponse>("/auth/steam/start");
    window.location.assign(result.authorizationUrl);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Could not start Steam login";
    setStatusMessage(message, true);
    setPendingState(false);
  }
};

steamButtonElement.addEventListener("click", () => {
  void startSteamLogin();
});

revealAuthPanel();
applySteamCallbackStatusFromQuery();
void refreshSession();
