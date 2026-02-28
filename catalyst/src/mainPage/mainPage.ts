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

const BACKEND_BASE_URL = "http://localhost:4000";

const statusElement = document.getElementById("main-status");

if (!(statusElement instanceof HTMLElement)) {
  throw new Error("Main page status element is missing");
}

const setStatus = (message: string, isError = false): void => {
  statusElement.textContent = message;
  statusElement.classList.toggle("status-error", isError);
};

const loadSession = async (): Promise<void> => {
  try {
    const response = await fetch(`${BACKEND_BASE_URL}/auth/session`, {
      method: "GET",
      credentials: "include"
    });

    if (!response.ok) {
      window.location.replace("/");
      return;
    }

    const session = (await response.json()) as SessionResponse;
    setStatus(`Signed in as ${session.user.email}. Steam ID: ${session.user.steamId ?? "not linked"}.`);
  } catch {
    setStatus("Could not load session data.", true);
  }
};

void loadSession();
