interface LibrarySyncState {
  isLoadingLibrary: boolean;
  lastLibraryRefreshAtMs: number | null;
  libraryLastUpdatedTimer: number | null;
}

interface LibraryStatusViewOptions {
  libraryLastUpdatedElement: HTMLElement;
  refreshLibraryButton: HTMLButtonElement;
  refreshLibraryLabelElement: HTMLElement;
  state: LibrarySyncState;
  formatLibraryRefreshAgeLabel: (ageMs: number) => string;
}

interface LibraryStatusView {
  renderLibraryLastUpdated: () => void;
  markLibraryAsUpdatedNow: () => void;
  stopLibraryLastUpdatedTimer: () => void;
  setLibraryLoadingState: (isLoading: boolean) => void;
}

export const createLibraryStatusView = ({
  libraryLastUpdatedElement,
  refreshLibraryButton,
  refreshLibraryLabelElement,
  state,
  formatLibraryRefreshAgeLabel,
}: LibraryStatusViewOptions): LibraryStatusView => {
  const renderLibraryLastUpdated = (): void => {
    if (state.isLoadingLibrary) {
      libraryLastUpdatedElement.textContent = "Syncing...";
      return;
    }

    if (state.lastLibraryRefreshAtMs === null) {
      libraryLastUpdatedElement.textContent = "Not synced yet";
      return;
    }

    libraryLastUpdatedElement.textContent = formatLibraryRefreshAgeLabel(Date.now() - state.lastLibraryRefreshAtMs);
  };

  const markLibraryAsUpdatedNow = (): void => {
    state.lastLibraryRefreshAtMs = Date.now();
    renderLibraryLastUpdated();
    if (state.libraryLastUpdatedTimer !== null) {
      return;
    }

    state.libraryLastUpdatedTimer = window.setInterval(() => {
      renderLibraryLastUpdated();
    }, 15000);
  };

  const stopLibraryLastUpdatedTimer = (): void => {
    if (state.libraryLastUpdatedTimer === null) {
      return;
    }

    window.clearInterval(state.libraryLastUpdatedTimer);
    state.libraryLastUpdatedTimer = null;
  };

  const setLibraryLoadingState = (isLoading: boolean): void => {
    state.isLoadingLibrary = isLoading;
    refreshLibraryButton.disabled = isLoading;
    refreshLibraryButton.classList.toggle("is-loading", isLoading);
    refreshLibraryButton.setAttribute("aria-busy", `${isLoading}`);
    refreshLibraryLabelElement.textContent = isLoading ? "Syncing" : "Refresh";
    renderLibraryLastUpdated();
  };

  return {
    renderLibraryLastUpdated,
    markLibraryAsUpdatedNow,
    stopLibraryLastUpdatedTimer,
    setLibraryLoadingState,
  };
};
