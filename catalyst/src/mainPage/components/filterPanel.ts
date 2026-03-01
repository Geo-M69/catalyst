import type {
  FilterByOption,
  GameKindFilter,
  GenreFilter,
  LibraryFilters,
  PlatformFilter,
  SortOption,
  SourceFilter,
} from "../types";

const DEFAULT_FILTERS: LibraryFilters = {
  search: "",
  steamTag: "",
  collection: "",
  filterBy: "all",
  platform: "all",
  source: "all",
  kind: "all",
  genre: "all",
  sortBy: "alphabetical",
};

interface BaseCustomSelectController {
  closeMenu: () => void;
}

interface CustomSelectOption<T extends string> {
  value: T;
  label: string;
}

interface CustomSelectController<T extends string> extends BaseCustomSelectController {
  getValue: () => T;
  setValue: (value: T, notifyChange?: boolean) => boolean;
  setOptions: (options: CustomSelectOption<T>[], preferredValue?: T, notifyChange?: boolean) => void;
}

const FILTER_TEMPLATE = `
  <form id="library-filter-form" class="filter-form filter-form-panel" autocomplete="off">
    <div class="filter-field">
      <label class="field-label filter-field-label" for="filter-search">Search</label>
      <input id="filter-search" class="text-input filter-input filter-input-search" type="search" placeholder="Search games..." />
    </div>

    <div id="steam-tag-field" class="filter-field filter-select-field">
      <label class="field-label filter-field-label" for="steam-tag-filter">Steam Store Tag</label>
      <button
        id="steam-tag-filter"
        class="text-input filter-input filter-select-trigger"
        type="button"
        aria-haspopup="listbox"
        aria-expanded="false"
        aria-controls="steam-tag-menu"
      >
        <span class="filter-select-trigger-text">All</span>
        <span class="filter-select-trigger-caret" aria-hidden="true"></span>
      </button>
      <div id="steam-tag-menu" class="filter-select-menu" role="listbox" hidden>
        <button type="button" class="filter-select-option" role="option" data-value="">All</button>
      </div>
    </div>

    <div id="collection-field" class="filter-field filter-select-field">
      <label class="field-label filter-field-label" for="collection-filter">Collection</label>
      <button
        id="collection-filter"
        class="text-input filter-input filter-select-trigger"
        type="button"
        aria-haspopup="listbox"
        aria-expanded="false"
        aria-controls="collection-menu"
      >
        <span class="filter-select-trigger-text">All</span>
        <span class="filter-select-trigger-caret" aria-hidden="true"></span>
      </button>
      <div id="collection-menu" class="filter-select-menu" role="listbox" hidden>
        <button type="button" class="filter-select-option" role="option" data-value="">All</button>
      </div>
    </div>

    <div id="filter-by-field" class="filter-field filter-select-field">
      <label class="field-label filter-field-label" for="filter-by-select">Filter By</label>
      <button
        id="filter-by-select"
        class="text-input filter-input filter-select-trigger"
        type="button"
        aria-haspopup="listbox"
        aria-expanded="false"
        aria-controls="filter-by-menu"
      >
        <span class="filter-select-trigger-text">All</span>
        <span class="filter-select-trigger-caret" aria-hidden="true"></span>
      </button>
      <div id="filter-by-menu" class="filter-select-menu" role="listbox" hidden>
        <button type="button" class="filter-select-option" role="option" data-value="all">All</button>
        <button type="button" class="filter-select-option" role="option" data-value="installed">Installed</button>
        <button type="button" class="filter-select-option" role="option" data-value="not-installed">Not Installed</button>
        <button type="button" class="filter-select-option" role="option" data-value="favorites">Favorites</button>
        <button type="button" class="filter-select-option" role="option" data-value="recently-played">Recently Played</button>
        <button type="button" class="filter-select-option" role="option" data-value="never-played">Never Played</button>
      </div>
    </div>

    <div id="platform-field" class="filter-field filter-select-field">
      <label class="field-label filter-field-label" for="platform-filter">Platform</label>
      <button
        id="platform-filter"
        class="text-input filter-input filter-select-trigger"
        type="button"
        aria-haspopup="listbox"
        aria-expanded="false"
        aria-controls="platform-menu"
      >
        <span class="filter-select-trigger-text">All</span>
        <span class="filter-select-trigger-caret" aria-hidden="true"></span>
      </button>
      <div id="platform-menu" class="filter-select-menu" role="listbox" hidden>
        <button type="button" class="filter-select-option" role="option" data-value="all">All</button>
        <button type="button" class="filter-select-option" role="option" data-value="windows">Windows</button>
        <button type="button" class="filter-select-option" role="option" data-value="macos">MacOS</button>
        <button type="button" class="filter-select-option" role="option" data-value="linux">Linux</button>
      </div>
    </div>

    <div id="source-field" class="filter-field filter-select-field">
      <label class="field-label filter-field-label" for="source-filter">Source</label>
      <button
        id="source-filter"
        class="text-input filter-input filter-select-trigger"
        type="button"
        aria-haspopup="listbox"
        aria-expanded="false"
        aria-controls="source-menu"
      >
        <span class="filter-select-trigger-text">All</span>
        <span class="filter-select-trigger-caret" aria-hidden="true"></span>
      </button>
      <div id="source-menu" class="filter-select-menu" role="listbox" hidden>
        <button type="button" class="filter-select-option" role="option" data-value="all">All</button>
        <button type="button" class="filter-select-option" role="option" data-value="steam">Steam</button>
        <button type="button" class="filter-select-option" role="option" data-value="epic-games">Epic Games</button>
      </div>
    </div>

    <div id="kind-field" class="filter-field filter-select-field">
      <label class="field-label filter-field-label" for="kind-filter">Type</label>
      <button
        id="kind-filter"
        class="text-input filter-input filter-select-trigger"
        type="button"
        aria-haspopup="listbox"
        aria-expanded="false"
        aria-controls="kind-menu"
      >
        <span class="filter-select-trigger-text">All</span>
        <span class="filter-select-trigger-caret" aria-hidden="true"></span>
      </button>
      <div id="kind-menu" class="filter-select-menu" role="listbox" hidden>
        <button type="button" class="filter-select-option" role="option" data-value="all">All</button>
        <button type="button" class="filter-select-option" role="option" data-value="game">Games</button>
        <button type="button" class="filter-select-option" role="option" data-value="demo">Demos</button>
        <button type="button" class="filter-select-option" role="option" data-value="dlc">DLCs</button>
        <button type="button" class="filter-select-option" role="option" data-value="unknown">Unknown</button>
      </div>
    </div>

    <div id="genre-field" class="filter-field filter-select-field">
      <label class="field-label filter-field-label" for="genre-filter">Genre</label>
      <button
        id="genre-filter"
        class="text-input filter-input filter-select-trigger"
        type="button"
        aria-haspopup="listbox"
        aria-expanded="false"
        aria-controls="genre-menu"
      >
        <span class="filter-select-trigger-text">All</span>
        <span class="filter-select-trigger-caret" aria-hidden="true"></span>
      </button>
      <div id="genre-menu" class="filter-select-menu" role="listbox" hidden>
        <button type="button" class="filter-select-option" role="option" data-value="all">All</button>
        <button type="button" class="filter-select-option" role="option" data-value="action">Action</button>
        <button type="button" class="filter-select-option" role="option" data-value="rpg">RPG</button>
        <button type="button" class="filter-select-option" role="option" data-value="strategy">Strategy</button>
        <button type="button" class="filter-select-option" role="option" data-value="simulation">Simulation</button>
        <button type="button" class="filter-select-option" role="option" data-value="fps">FPS</button>
      </div>
    </div>

    <div id="sort-by-field" class="filter-field filter-select-field">
      <label class="field-label filter-field-label" for="sort-by-filter">Sort By</label>
      <button
        id="sort-by-filter"
        class="text-input filter-input filter-select-trigger"
        type="button"
        aria-haspopup="listbox"
        aria-expanded="false"
        aria-controls="sort-by-menu"
      >
        <span class="filter-select-trigger-text">Alphabetical</span>
        <span class="filter-select-trigger-caret" aria-hidden="true"></span>
      </button>
      <div id="sort-by-menu" class="filter-select-menu" role="listbox" hidden>
        <button type="button" class="filter-select-option" role="option" data-value="alphabetical">Alphabetical</button>
        <button type="button" class="filter-select-option" role="option" data-value="alphabetical-reverse">Alphabetical (reverse)</button>
        <button type="button" class="filter-select-option" role="option" data-value="least-played">Least Played</button>
        <button type="button" class="filter-select-option" role="option" data-value="most-played">Most Played</button>
      </div>
    </div>

    <button id="clear-filters-button" class="secondary-button filter-clear-button" type="button">Clear Filters</button>
  </form>
`;

const createCustomSelect = <T extends string>(
  field: HTMLElement,
  onChange: () => void,
  closeOtherMenus: () => void
): CustomSelectController<T> => {
  const trigger = field.querySelector(".filter-select-trigger");
  const triggerText = field.querySelector(".filter-select-trigger-text");
  const menu = field.querySelector(".filter-select-menu");

  if (
    !(trigger instanceof HTMLButtonElement)
    || !(triggerText instanceof HTMLElement)
    || !(menu instanceof HTMLElement)
  ) {
    throw new Error("Custom select is missing required DOM elements");
  }

  let optionButtons: HTMLButtonElement[] = [];
  let currentValue = "" as T;

  const closeMenu = (): void => {
    menu.hidden = true;
    field.classList.remove("is-open");
    trigger.setAttribute("aria-expanded", "false");
  };

  const openMenu = (): void => {
    closeOtherMenus();
    menu.hidden = false;
    field.classList.add("is-open");
    trigger.setAttribute("aria-expanded", "true");
  };

  const focusCurrentOption = (): void => {
    if (optionButtons.length === 0) {
      return;
    }
    const selectedOption = optionButtons.find((optionButton) => optionButton.classList.contains("is-selected")) ?? optionButtons[0];
    selectedOption.focus();
  };

  const setValue = (value: T, notifyChange = true): boolean => {
    const selectedOption = optionButtons.find((optionButton) => optionButton.dataset.value === value);
    if (!selectedOption) {
      return false;
    }

    currentValue = value;
    triggerText.textContent = selectedOption.textContent?.trim() ?? "";

    for (const optionButton of optionButtons) {
      const isSelected = optionButton === selectedOption;
      optionButton.classList.toggle("is-selected", isSelected);
      optionButton.setAttribute("aria-selected", `${isSelected}`);
    }

    if (notifyChange) {
      onChange();
    }

    return true;
  };

  const bindOptionButton = (optionButton: HTMLButtonElement): void => {
    if (optionButton.dataset.bound === "true") {
      return;
    }

    optionButton.dataset.bound = "true";
    optionButton.addEventListener("click", () => {
      const optionValue = optionButton.dataset.value;
      if (optionValue === undefined) {
        return;
      }

      void setValue(optionValue as T);
      closeMenu();
      trigger.focus();
    });
  };

  const setOptions = (
    options: CustomSelectOption<T>[],
    preferredValue?: T,
    notifyChange = false
  ): void => {
    if (options.length === 0) {
      throw new Error("Custom select must include at least one option with a value");
    }

    const fragment = document.createDocumentFragment();
    for (const option of options) {
      const optionButton = document.createElement("button");
      optionButton.type = "button";
      optionButton.className = "filter-select-option";
      optionButton.setAttribute("role", "option");
      optionButton.dataset.value = option.value;
      optionButton.textContent = option.label;
      bindOptionButton(optionButton);
      fragment.append(optionButton);
    }

    menu.replaceChildren(fragment);
    optionButtons = Array.from(menu.querySelectorAll(".filter-select-option")) as HTMLButtonElement[];

    const candidateValue = preferredValue ?? currentValue;
    const hasCandidateValue = optionButtons.some((optionButton) => optionButton.dataset.value === candidateValue);
    const nextValue = hasCandidateValue ? candidateValue : options[0].value;
    const shouldNotify = notifyChange && nextValue !== currentValue;
    void setValue(nextValue, shouldNotify);
  };

  trigger.addEventListener("click", () => {
    if (menu.hidden) {
      openMenu();
      focusCurrentOption();
      return;
    }

    closeMenu();
  });

  trigger.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      closeMenu();
      return;
    }

    if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      openMenu();
      focusCurrentOption();
    }
  });

  menu.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      closeMenu();
      trigger.focus();
      return;
    }

    const activeElement = document.activeElement;
    if (!(activeElement instanceof HTMLButtonElement)) {
      return;
    }

    const focusedIndex = optionButtons.indexOf(activeElement);
    if (focusedIndex < 0) {
      return;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      const nextIndex = Math.min(focusedIndex + 1, optionButtons.length - 1);
      optionButtons[nextIndex].focus();
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      const previousIndex = Math.max(focusedIndex - 1, 0);
      optionButtons[previousIndex].focus();
      return;
    }

    if (event.key === "Home") {
      event.preventDefault();
      optionButtons[0].focus();
      return;
    }

    if (event.key === "End") {
      event.preventDefault();
      optionButtons[optionButtons.length - 1].focus();
      return;
    }

    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      activeElement.click();
    }
  });

  document.addEventListener("pointerdown", (event) => {
    const eventTarget = event.target;
    if (eventTarget instanceof Node && !field.contains(eventTarget)) {
      closeMenu();
    }
  });

  const initialOptionNodes = Array.from(menu.querySelectorAll(".filter-select-option"));
  const areOptionButtons = initialOptionNodes.every((optionNode) => optionNode instanceof HTMLButtonElement);
  if (!areOptionButtons) {
    throw new Error("Custom select options must be buttons");
  }
  const initialOptions = (initialOptionNodes as HTMLButtonElement[]).map((optionButton) => {
    const value = optionButton.dataset.value;
    if (value === undefined) {
      throw new Error("Custom select option is missing a value");
    }

    return {
      value: value as T,
      label: optionButton.textContent?.trim() ?? value,
    };
  });
  setOptions(initialOptions, undefined, false);

  return {
    getValue: (): T => currentValue as T,
    setValue,
    setOptions,
    closeMenu,
  };
};

export interface FilterPanelController {
  getFilters: () => LibraryFilters;
  setFilterBy: (filterBy: FilterByOption, notifyChange?: boolean) => boolean;
  setSteamTagSuggestions: (tags: string[]) => void;
  setCollectionSuggestions: (collections: string[]) => void;
  setCollectionFilter: (collection: string, notifyChange?: boolean) => boolean;
}

export const createFilterPanel = (
  container: HTMLElement,
  onChange: () => void
): FilterPanelController => {
  container.innerHTML = FILTER_TEMPLATE;

  const searchInput = container.querySelector("#filter-search");
  const steamTagField = container.querySelector("#steam-tag-field");
  const collectionField = container.querySelector("#collection-field");
  const filterByField = container.querySelector("#filter-by-field");
  const platformField = container.querySelector("#platform-field");
  const sourceField = container.querySelector("#source-field");
  const kindField = container.querySelector("#kind-field");
  const genreField = container.querySelector("#genre-field");
  const sortByField = container.querySelector("#sort-by-field");
  const clearButton = container.querySelector("#clear-filters-button");

  if (
    !(searchInput instanceof HTMLInputElement)
    || !(steamTagField instanceof HTMLElement)
    || !(collectionField instanceof HTMLElement)
    || !(filterByField instanceof HTMLElement)
    || !(platformField instanceof HTMLElement)
    || !(sourceField instanceof HTMLElement)
    || !(kindField instanceof HTMLElement)
    || !(genreField instanceof HTMLElement)
    || !(sortByField instanceof HTMLElement)
    || !(clearButton instanceof HTMLButtonElement)
  ) {
    throw new Error("Filter panel is missing required DOM elements");
  }

  const selectControllers: BaseCustomSelectController[] = [];
  const closeAllSelectMenus = (): void => {
    for (const selectController of selectControllers) {
      selectController.closeMenu();
    }
  };

  const notifyChange = (): void => {
    onChange();
  };

  const steamTagSelect = createCustomSelect<string>(steamTagField, notifyChange, closeAllSelectMenus);
  const collectionSelect = createCustomSelect<string>(collectionField, notifyChange, closeAllSelectMenus);
  const filterBySelect = createCustomSelect<FilterByOption>(filterByField, notifyChange, closeAllSelectMenus);
  const platformSelect = createCustomSelect<PlatformFilter>(platformField, notifyChange, closeAllSelectMenus);
  const sourceSelect = createCustomSelect<SourceFilter>(sourceField, notifyChange, closeAllSelectMenus);
  const kindSelect = createCustomSelect<GameKindFilter>(kindField, notifyChange, closeAllSelectMenus);
  const genreSelect = createCustomSelect<GenreFilter>(genreField, notifyChange, closeAllSelectMenus);
  const sortBySelect = createCustomSelect<SortOption>(sortByField, notifyChange, closeAllSelectMenus);

  selectControllers.push(
    steamTagSelect,
    collectionSelect,
    filterBySelect,
    platformSelect,
    sourceSelect,
    kindSelect,
    genreSelect,
    sortBySelect
  );

  const buildFiltersFromForm = (): LibraryFilters => {
    return {
      search: searchInput.value,
      steamTag: steamTagSelect.getValue(),
      collection: collectionSelect.getValue(),
      filterBy: filterBySelect.getValue(),
      platform: platformSelect.getValue(),
      source: sourceSelect.getValue(),
      kind: kindSelect.getValue(),
      genre: genreSelect.getValue(),
      sortBy: sortBySelect.getValue(),
    };
  };

  searchInput.addEventListener("input", notifyChange);

  clearButton.addEventListener("click", () => {
    searchInput.value = DEFAULT_FILTERS.search;
    steamTagSelect.setValue(DEFAULT_FILTERS.steamTag, false);
    collectionSelect.setValue(DEFAULT_FILTERS.collection, false);
    filterBySelect.setValue(DEFAULT_FILTERS.filterBy, false);
    platformSelect.setValue(DEFAULT_FILTERS.platform, false);
    sourceSelect.setValue(DEFAULT_FILTERS.source, false);
    kindSelect.setValue(DEFAULT_FILTERS.kind, false);
    genreSelect.setValue(DEFAULT_FILTERS.genre, false);
    sortBySelect.setValue(DEFAULT_FILTERS.sortBy, false);
    closeAllSelectMenus();

    onChange();
  });

  return {
    getFilters: buildFiltersFromForm,
    setFilterBy: (filterBy: FilterByOption, notifyChange = true) => {
      return filterBySelect.setValue(filterBy, notifyChange);
    },
    setSteamTagSuggestions: (tags: string[]) => {
      const options: CustomSelectOption<string>[] = [
        { value: "", label: "All" },
        ...tags.map((tag) => ({ value: tag, label: tag })),
      ];
      steamTagSelect.setOptions(options, steamTagSelect.getValue(), false);
    },
    setCollectionSuggestions: (collections: string[]) => {
      const options: CustomSelectOption<string>[] = [
        { value: "", label: "All" },
        ...collections.map((collection) => ({ value: collection, label: collection })),
      ];
      collectionSelect.setOptions(options, collectionSelect.getValue(), false);
    },
    setCollectionFilter: (collection: string, notifyChange = true) => {
      const normalizedCollection = collection.trim();
      const nextValue = normalizedCollection.length > 0 ? normalizedCollection : DEFAULT_FILTERS.collection;
      return collectionSelect.setValue(nextValue, notifyChange);
    },
  };
};
