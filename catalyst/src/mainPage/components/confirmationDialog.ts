export interface ConfirmationDialogController {
  close: () => void;
  open: (options: ConfirmationDialogOpenOptions) => Promise<boolean>;
}

interface ConfirmationDialogOpenOptions {
  cancelLabel?: string;
  confirmLabel?: string;
  confirmTone?: "default" | "danger";
  description: string;
  title: string;
}

const DEFAULT_CONFIRMATION_OPTIONS = {
  cancelLabel: "Cancel",
  confirmLabel: "Confirm",
  confirmTone: "default" as const,
};

export const createConfirmationDialog = (): ConfirmationDialogController => {
  const backdrop = document.createElement("div");
  backdrop.className = "collection-confirm-dialog-backdrop";
  backdrop.hidden = true;

  const panel = document.createElement("section");
  panel.className = "collection-confirm-dialog-panel";
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-modal", "true");
  panel.setAttribute("aria-labelledby", "collection-confirm-dialog-title");

  const title = document.createElement("h3");
  title.id = "collection-confirm-dialog-title";
  title.className = "collection-confirm-dialog-title";

  const description = document.createElement("p");
  description.className = "collection-confirm-dialog-description";

  const actions = document.createElement("div");
  actions.className = "collection-confirm-dialog-actions";

  const cancelButton = document.createElement("button");
  cancelButton.type = "button";
  cancelButton.className = "collection-confirm-dialog-button secondary-button";
  cancelButton.textContent = DEFAULT_CONFIRMATION_OPTIONS.cancelLabel;

  const confirmButton = document.createElement("button");
  confirmButton.type = "button";
  confirmButton.className = "collection-confirm-dialog-button collection-confirm-dialog-button-confirm";
  confirmButton.textContent = DEFAULT_CONFIRMATION_OPTIONS.confirmLabel;

  actions.append(cancelButton, confirmButton);
  panel.append(title, description, actions);
  backdrop.append(panel);
  document.body.append(backdrop);

  let resolver: ((value: boolean) => void) | null = null;

  const finish = (value: boolean): void => {
    if (resolver) {
      resolver(value);
      resolver = null;
    }
    backdrop.hidden = true;
  };

  const close = (): void => {
    if (backdrop.hidden) {
      return;
    }
    finish(false);
  };

  cancelButton.addEventListener("click", close);
  confirmButton.addEventListener("click", () => {
    finish(true);
  });

  backdrop.addEventListener("pointerdown", (event) => {
    if (event.target === backdrop) {
      close();
    }
  });

  window.addEventListener("keydown", (event) => {
    if (!backdrop.hidden && event.key === "Escape") {
      event.preventDefault();
      close();
    }
  });

  return {
    close,
    open: (options) => {
      if (resolver) {
        finish(false);
      }

      const resolvedOptions = {
        ...DEFAULT_CONFIRMATION_OPTIONS,
        ...options,
      };

      title.textContent = resolvedOptions.title;
      description.textContent = resolvedOptions.description;
      cancelButton.textContent = resolvedOptions.cancelLabel;
      confirmButton.textContent = resolvedOptions.confirmLabel;
      confirmButton.classList.toggle(
        "is-danger",
        resolvedOptions.confirmTone === "danger"
      );

      backdrop.hidden = false;
      confirmButton.focus();

      return new Promise((resolve) => {
        resolver = resolve;
      });
    },
  };
};
