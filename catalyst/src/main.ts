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

const MAIN_PAGE_PATH = "/main.html";

const redirectToLibrary = (): void => {
  if (window.location.pathname === MAIN_PAGE_PATH) {
    return;
  }

  window.location.replace(MAIN_PAGE_PATH);
};

redirectToLibrary();
