export interface CollectionNameDialogController {
  close: () => void;
  open: (options?: CollectionNameDialogOpenOptions) => Promise<string | null>;
}

interface CollectionNameDialogOpenOptions {
  confirmLabel?: string;
  description?: string;
  initialValue?: string;
  placeholder?: string;
  title?: string;
}

const DEFAULT_OPEN_OPTIONS: Required<CollectionNameDialogOpenOptions> = {
  confirmLabel: "Create",
  description: "Name your new collection.",
  initialValue: "",
  placeholder: "Collection name",
  title: "Create Collection",
};

export const createCollectionNameDialog = (): CollectionNameDialogController => {
  const backdrop = document.createElement("div");
  backdrop.className = "collection-dialog-backdrop";
  backdrop.hidden = true;

  const panel = document.createElement("section");
  panel.className = "collection-dialog-panel";
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-modal", "true");
  panel.setAttribute("aria-labelledby", "collection-dialog-title");

  const title = document.createElement("h3");
  title.id = "collection-dialog-title";
  title.className = "collection-dialog-title";
  title.textContent = DEFAULT_OPEN_OPTIONS.title;

  const description = document.createElement("p");
  description.className = "collection-dialog-description";
  description.textContent = DEFAULT_OPEN_OPTIONS.description;

  const form = document.createElement("form");
  form.className = "collection-dialog-form";

  const input = document.createElement("input");
  input.className = "collection-dialog-input text-input";
  input.type = "text";
  input.maxLength = 80;
  input.placeholder = DEFAULT_OPEN_OPTIONS.placeholder;
  input.autocomplete = "off";

  const errorText = document.createElement("p");
  errorText.className = "collection-dialog-error";
  errorText.hidden = true;

  const actions = document.createElement("div");
  actions.className = "collection-dialog-actions";

  const cancelButton = document.createElement("button");
  cancelButton.type = "button";
  cancelButton.className = "collection-dialog-cancel secondary-button";
  cancelButton.textContent = "Cancel";

  const createButton = document.createElement("button");
  createButton.type = "submit";
  createButton.className = "collection-dialog-create secondary-button";
  createButton.textContent = DEFAULT_OPEN_OPTIONS.confirmLabel;

  actions.append(cancelButton, createButton);
  form.append(input, errorText, actions);
  panel.append(title, description, form);
  backdrop.append(panel);
  document.body.append(backdrop);

  let resolver: ((value: string | null) => void) | null = null;

  const finish = (value: string | null): void => {
    if (resolver) {
      resolver(value);
      resolver = null;
    }
    backdrop.hidden = true;
  };

  const setError = (message: string | null): void => {
    if (message) {
      errorText.textContent = message;
      errorText.hidden = false;
      return;
    }

    errorText.hidden = true;
    errorText.textContent = "";
  };

  const close = (): void => {
    if (backdrop.hidden) {
      return;
    }
    finish(null);
  };

  const submit = (): void => {
    const name = input.value.trim();
    if (name.length === 0) {
      setError("Collection name cannot be empty.");
      input.focus();
      return;
    }

    setError(null);
    finish(name);
  };

  cancelButton.addEventListener("click", close);

  backdrop.addEventListener("pointerdown", (event) => {
    if (event.target === backdrop) {
      close();
    }
  });

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    submit();
  });

  window.addEventListener("keydown", (event) => {
    if (!backdrop.hidden && event.key === "Escape") {
      event.preventDefault();
      close();
    }
  });

  return {
    close,
    open: (options = {}) => {
      if (resolver) {
        finish(null);
      }

      const resolvedOptions = {
        ...DEFAULT_OPEN_OPTIONS,
        ...options,
      };
      backdrop.hidden = false;
      title.textContent = resolvedOptions.title;
      description.textContent = resolvedOptions.description;
      createButton.textContent = resolvedOptions.confirmLabel;
      input.placeholder = resolvedOptions.placeholder;
      input.value = resolvedOptions.initialValue;
      setError(null);
      input.setSelectionRange(0, input.value.length);
      input.focus();

      return new Promise((resolve) => {
        resolver = resolve;
      });
    },
  };
};
