interface PlaceholderOption {
  title: string;
  description: string;
}

const DEFAULT_OPTIONS: PlaceholderOption[] = [
  {
    title: "Download Path",
    description: "Choose where newly installed games will be stored.",
  },
  {
    title: "Startup Behavior",
    description: "Control auto-launch and background startup preferences.",
  },
  {
    title: "Performance Mode",
    description: "Set launcher behavior for low-power devices.",
  },
  {
    title: "Notifications",
    description: "Configure alerts for updates and downloads.",
  },
  {
    title: "Connected Accounts",
    description: "Manage linked stores and platform integrations.",
  },
];

export const renderOptionsPanel = (
  container: HTMLElement,
  options: PlaceholderOption[] = DEFAULT_OPTIONS
): void => {
  container.replaceChildren();

  for (const option of options) {
    const item = document.createElement("li");
    item.className = "option-item";

    const title = document.createElement("h3");
    title.className = "option-title";
    title.textContent = option.title;

    const description = document.createElement("p");
    description.className = "option-description";
    description.textContent = option.description;

    item.append(title, description);
    container.append(item);
  }
};
