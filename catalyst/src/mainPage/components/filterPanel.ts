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

type FilterKey = keyof LibraryFilters;

interface ActiveFilterChip {
  key: FilterKey;
  label: string;
}

interface BaseCustomSelectController {
  closeMenu: () => void;
}

interface CustomSelectOption<T extends string> {
  value: T;
  label: string;
}

interface CustomSelectController<T extends string> extends BaseCustomSelectController {
  getValue: () => T;
  getLabel: () => string;
  setValue: (value: T, notifyChange?: boolean) => boolean;
  setOptions: (options: CustomSelectOption<T>[], preferredValue?: T, notifyChange?: boolean) => void;
}

const FILTER_TEMPLATE = `
  <form id="library-filter-form" class="filter-form filter-form-panel" autocomplete="off">
    <section class="filter-section filter-quick-section" data-filter-keys="search,platform,genre,sortBy">
      <div class="filter-active-stack">
        <div id="filter-active-chip-list" class="filter-active-chip-list" role="list"></div>
      </div>

      <div class="filter-section-content">
        <div class="filter-field">
          <input
            id="filter-search"
            class="text-input filter-input filter-input-search"
            type="search"
            placeholder="Search games..."
            aria-label="Search"
          />
        </div>

        <div id="platform-field" class="filter-field filter-select-field">
          <button
            id="platform-filter"
            class="text-input filter-input filter-select-trigger"
            type="button"
            aria-label="Platform"
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

        <div id="genre-field" class="filter-field filter-select-field">
          <button
            id="genre-filter"
            class="text-input filter-input filter-select-trigger"
            type="button"
            aria-label="Genre"
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
          <button
            id="sort-by-filter"
            class="text-input filter-input filter-select-trigger"
            type="button"
            aria-label="Sort"
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
      </div>
    </section>

    <div class="filter-advanced-dropdown">
      <button
        id="filter-advanced-toggle"
        class="filter-advanced-toggle"
        type="button"
        aria-expanded="false"
        aria-controls="filter-advanced-content"
      >
        <span class="filter-section-kicker">Advanced</span>
        <span class="filter-advanced-toggle-caret" aria-hidden="true"></span>
      </button>

      <div id="filter-advanced-content" class="filter-advanced-content" hidden>
        <section class="filter-section filter-section-collapsible is-open" data-filter-keys="collection,filterBy">
          <button
            type="button"
            class="filter-section-toggle"
            data-filter-section-toggle
            data-target="ownership-section-content"
            aria-expanded="true"
            aria-controls="ownership-section-content"
          >
            <span class="filter-section-toggle-copy">
              <span class="filter-section-toggle-title">Ownership</span>
              <span class="filter-section-toggle-caption">Collection and status</span>
            </span>
            <span class="filter-section-toggle-active-count" hidden>0 active</span>
            <span class="filter-section-toggle-caret" aria-hidden="true"></span>
          </button>
          <div id="ownership-section-content" class="filter-section-content">
            <div id="collection-field" class="filter-field filter-select-field">
              <button
                id="collection-filter"
                class="text-input filter-input filter-select-trigger"
                type="button"
                aria-label="Collection"
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
              <button
                id="filter-by-select"
                class="text-input filter-input filter-select-trigger"
                type="button"
                aria-label="Status"
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
          </div>
        </section>

        <section class="filter-section filter-section-collapsible" data-filter-keys="steamTag,source">
          <button
            type="button"
            class="filter-section-toggle"
            data-filter-section-toggle
            data-target="store-section-content"
            aria-expanded="false"
            aria-controls="store-section-content"
          >
            <span class="filter-section-toggle-copy">
              <span class="filter-section-toggle-title">Store</span>
              <span class="filter-section-toggle-caption">Tags and provider</span>
            </span>
            <span class="filter-section-toggle-active-count" hidden>0 active</span>
            <span class="filter-section-toggle-caret" aria-hidden="true"></span>
          </button>
          <div id="store-section-content" class="filter-section-content" hidden>
            <div id="steam-tag-field" class="filter-field filter-select-field">
              <button
                id="steam-tag-filter"
                class="text-input filter-input filter-select-trigger"
                type="button"
                aria-label="Steam store tag"
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

            <div id="source-field" class="filter-field filter-select-field">
              <button
                id="source-filter"
                class="text-input filter-input filter-select-trigger"
                type="button"
                aria-label="Source"
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
          </div>
        </section>

        <section class="filter-section filter-section-collapsible" data-filter-keys="kind">
          <button
            type="button"
            class="filter-section-toggle"
            data-filter-section-toggle
            data-target="metadata-section-content"
            aria-expanded="false"
            aria-controls="metadata-section-content"
          >
            <span class="filter-section-toggle-copy">
              <span class="filter-section-toggle-title">Metadata</span>
              <span class="filter-section-toggle-caption">Content type</span>
            </span>
            <span class="filter-section-toggle-active-count" hidden>0 active</span>
            <span class="filter-section-toggle-caret" aria-hidden="true"></span>
          </button>
          <div id="metadata-section-content" class="filter-section-content" hidden>
            <div id="kind-field" class="filter-field filter-select-field">
              <button
                id="kind-filter"
                class="text-input filter-input filter-select-trigger"
                type="button"
                aria-label="Type"
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
          </div>
        </section>

        <button id="clear-filters-button" class="secondary-button filter-clear-button" type="button" hidden>Clear all</button>
      </div>
    </div>
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
    getLabel: (): string => triggerText.textContent?.trim() ?? "",
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
  const platformField = container.querySelector("#platform-field");
  const genreField = container.querySelector("#genre-field");
  const sortByField = container.querySelector("#sort-by-field");
  const advancedToggle = container.querySelector("#filter-advanced-toggle");
  const advancedContent = container.querySelector("#filter-advanced-content");
  const activeStack = container.querySelector(".filter-active-stack");
  const activeChipList = container.querySelector("#filter-active-chip-list");
  const sectionToggles = Array.from(container.querySelectorAll("[data-filter-section-toggle]"))
    .filter((toggleButton): toggleButton is HTMLButtonElement => toggleButton instanceof HTMLButtonElement);
  const filterSections = Array.from(container.querySelectorAll(".filter-section[data-filter-keys]"))
    .filter((section): section is HTMLElement => section instanceof HTMLElement);
  const steamTagField = container.querySelector("#steam-tag-field");
  const collectionField = container.querySelector("#collection-field");
  const filterByField = container.querySelector("#filter-by-field");
  const sourceField = container.querySelector("#source-field");
  const kindField = container.querySelector("#kind-field");
  const clearButton = container.querySelector("#clear-filters-button");

  if (
    !(searchInput instanceof HTMLInputElement)
    || !(platformField instanceof HTMLElement)
    || !(genreField instanceof HTMLElement)
    || !(sortByField instanceof HTMLElement)
    || !(advancedToggle instanceof HTMLButtonElement)
    || !(advancedContent instanceof HTMLElement)
    || !(activeStack instanceof HTMLElement)
    || !(activeChipList instanceof HTMLElement)
    || !(steamTagField instanceof HTMLElement)
    || !(collectionField instanceof HTMLElement)
    || !(filterByField instanceof HTMLElement)
    || !(sourceField instanceof HTMLElement)
    || !(kindField instanceof HTMLElement)
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

  let handleSelectChange = (): void => {};

  const steamTagSelect = createCustomSelect<string>(steamTagField, () => handleSelectChange(), closeAllSelectMenus);
  const collectionSelect = createCustomSelect<string>(collectionField, () => handleSelectChange(), closeAllSelectMenus);
  const filterBySelect = createCustomSelect<FilterByOption>(filterByField, () => handleSelectChange(), closeAllSelectMenus);
  const platformSelect = createCustomSelect<PlatformFilter>(platformField, () => handleSelectChange(), closeAllSelectMenus);
  const sourceSelect = createCustomSelect<SourceFilter>(sourceField, () => handleSelectChange(), closeAllSelectMenus);
  const kindSelect = createCustomSelect<GameKindFilter>(kindField, () => handleSelectChange(), closeAllSelectMenus);
  const genreSelect = createCustomSelect<GenreFilter>(genreField, () => handleSelectChange(), closeAllSelectMenus);
  const sortBySelect = createCustomSelect<SortOption>(sortByField, () => handleSelectChange(), closeAllSelectMenus);

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

  const isFilterKey = (value: string): value is FilterKey => {
    return Object.prototype.hasOwnProperty.call(DEFAULT_FILTERS, value);
  };

  const isFilterKeyActive = (key: FilterKey): boolean => {
    if (key === "search") {
      return searchInput.value.trim().length > 0;
    }

    return buildFiltersFromForm()[key] !== DEFAULT_FILTERS[key];
  };

  const resetFilterByKey = (key: FilterKey): void => {
    switch (key) {
      case "search":
        searchInput.value = DEFAULT_FILTERS.search;
        break;
      case "steamTag":
        steamTagSelect.setValue(DEFAULT_FILTERS.steamTag, false);
        break;
      case "collection":
        collectionSelect.setValue(DEFAULT_FILTERS.collection, false);
        break;
      case "filterBy":
        filterBySelect.setValue(DEFAULT_FILTERS.filterBy, false);
        break;
      case "platform":
        platformSelect.setValue(DEFAULT_FILTERS.platform, false);
        break;
      case "source":
        sourceSelect.setValue(DEFAULT_FILTERS.source, false);
        break;
      case "kind":
        kindSelect.setValue(DEFAULT_FILTERS.kind, false);
        break;
      case "genre":
        genreSelect.setValue(DEFAULT_FILTERS.genre, false);
        break;
      case "sortBy":
        sortBySelect.setValue(DEFAULT_FILTERS.sortBy, false);
        break;
      default:
        break;
    }
  };

  const getActiveFilterChips = (): ActiveFilterChip[] => {
    const chips: ActiveFilterChip[] = [];
    const searchValue = searchInput.value.trim();
    if (searchValue.length > 0) {
      chips.push({ key: "search", label: `Search: ${searchValue}` });
    }

    if (steamTagSelect.getValue() !== DEFAULT_FILTERS.steamTag) {
      chips.push({ key: "steamTag", label: `Tag: ${steamTagSelect.getLabel()}` });
    }

    if (collectionSelect.getValue() !== DEFAULT_FILTERS.collection) {
      chips.push({ key: "collection", label: `Collection: ${collectionSelect.getLabel()}` });
    }

    if (filterBySelect.getValue() !== DEFAULT_FILTERS.filterBy) {
      chips.push({ key: "filterBy", label: `Status: ${filterBySelect.getLabel()}` });
    }

    if (platformSelect.getValue() !== DEFAULT_FILTERS.platform) {
      chips.push({ key: "platform", label: `Platform: ${platformSelect.getLabel()}` });
    }

    if (sourceSelect.getValue() !== DEFAULT_FILTERS.source) {
      chips.push({ key: "source", label: `Source: ${sourceSelect.getLabel()}` });
    }

    if (kindSelect.getValue() !== DEFAULT_FILTERS.kind) {
      chips.push({ key: "kind", label: `Type: ${kindSelect.getLabel()}` });
    }

    if (genreSelect.getValue() !== DEFAULT_FILTERS.genre) {
      chips.push({ key: "genre", label: `Genre: ${genreSelect.getLabel()}` });
    }

    if (sortBySelect.getValue() !== DEFAULT_FILTERS.sortBy) {
      chips.push({ key: "sortBy", label: `Sort: ${sortBySelect.getLabel()}` });
    }

    return chips;
  };

  const updateClearButton = (activeFilterCount: number): void => {
    if (activeFilterCount === 0) {
      clearButton.hidden = true;
      clearButton.textContent = "Clear all";
      return;
    }

    clearButton.hidden = false;
    clearButton.textContent = `Clear all (${activeFilterCount})`;
  };

  const updateSectionActivity = (): void => {
    for (const section of filterSections) {
      const keys = (section.dataset.filterKeys ?? "")
        .split(",")
        .map((key) => key.trim())
        .filter(isFilterKey);
      const activeKeyCount = keys.filter((key) => isFilterKeyActive(key)).length;
      section.classList.toggle("has-active-filters", activeKeyCount > 0);

      const activeCountBadge = section.querySelector(".filter-section-toggle-active-count");
      if (!(activeCountBadge instanceof HTMLElement)) {
        continue;
      }

      if (activeKeyCount === 0) {
        activeCountBadge.hidden = true;
        activeCountBadge.textContent = "0 active";
        continue;
      }

      activeCountBadge.hidden = false;
      activeCountBadge.textContent = `${activeKeyCount} active`;
    }
  };

  const renderActiveFilterChips = (): number => {
    const chips = getActiveFilterChips();
    activeChipList.replaceChildren();

    for (const chip of chips) {
      const chipButton = document.createElement("button");
      chipButton.type = "button";
      chipButton.className = "filter-active-chip";
      chipButton.setAttribute("role", "listitem");
      chipButton.setAttribute("aria-label", `Remove ${chip.label}`);

      const chipLabel = document.createElement("span");
      chipLabel.className = "filter-active-chip-label";
      chipLabel.textContent = chip.label;

      const chipRemove = document.createElement("span");
      chipRemove.className = "filter-active-chip-remove";
      chipRemove.setAttribute("aria-hidden", "true");
      chipRemove.textContent = "x";

      chipButton.append(chipLabel, chipRemove);
      chipButton.addEventListener("click", () => {
        resetFilterByKey(chip.key);
        closeAllSelectMenus();
        refreshFilterUi();
        onChange();
      });

      activeChipList.append(chipButton);
    }

    const hasActiveFilters = chips.length > 0;
    activeStack.hidden = !hasActiveFilters;
    activeChipList.hidden = !hasActiveFilters;
    return chips.length;
  };

  const refreshFilterUi = (): void => {
    const activeFilterCount = renderActiveFilterChips();
    updateClearButton(activeFilterCount);
    updateSectionActivity();
  };

  const notifyFilterChange = (): void => {
    refreshFilterUi();
    onChange();
  };
  handleSelectChange = notifyFilterChange;

  const setAdvancedExpandedState = (expanded: boolean): void => {
    advancedToggle.setAttribute("aria-expanded", `${expanded}`);
    advancedContent.hidden = !expanded;
  };

  setAdvancedExpandedState(advancedToggle.getAttribute("aria-expanded") === "true");

  advancedToggle.addEventListener("click", () => {
    closeAllSelectMenus();
    const isExpanded = advancedToggle.getAttribute("aria-expanded") === "true";
    setAdvancedExpandedState(!isExpanded);
  });

  for (const sectionToggle of sectionToggles) {
    const sectionContentId = sectionToggle.dataset.target;
    if (!sectionContentId) {
      continue;
    }
    const sectionContent = container.querySelector(`#${sectionContentId}`);
    const sectionRoot = sectionToggle.closest(".filter-section-collapsible");
    if (!(sectionContent instanceof HTMLElement) || !(sectionRoot instanceof HTMLElement)) {
      continue;
    }

    const setExpandedState = (expanded: boolean): void => {
      sectionToggle.setAttribute("aria-expanded", `${expanded}`);
      sectionContent.hidden = !expanded;
      sectionRoot.classList.toggle("is-open", expanded);
    };

    setExpandedState(sectionToggle.getAttribute("aria-expanded") === "true");

    sectionToggle.addEventListener("click", () => {
      closeAllSelectMenus();
      const isExpanded = sectionToggle.getAttribute("aria-expanded") === "true";
      setExpandedState(!isExpanded);
    });
  }

  searchInput.addEventListener("input", notifyFilterChange);

  const setSelectValue = <T extends string>(
    select: CustomSelectController<T>,
    value: T,
    notifyFilterUpdate = true
  ): boolean => {
    const wasApplied = select.setValue(value, false);
    if (!wasApplied) {
      return false;
    }

    if (notifyFilterUpdate) {
      notifyFilterChange();
      return true;
    }

    refreshFilterUi();
    return true;
  };

  clearButton.addEventListener("click", () => {
    for (const key of Object.keys(DEFAULT_FILTERS).filter(isFilterKey)) {
      resetFilterByKey(key);
    }
    closeAllSelectMenus();
    notifyFilterChange();
  });

  refreshFilterUi();

  return {
    getFilters: buildFiltersFromForm,
    setFilterBy: (filterBy: FilterByOption, shouldNotify = true) => {
      return setSelectValue(filterBySelect, filterBy, shouldNotify);
    },
    setSteamTagSuggestions: (tags: string[]) => {
      const options: CustomSelectOption<string>[] = [
        { value: "", label: "All" },
        ...tags.map((tag) => ({ value: tag, label: tag })),
      ];
      steamTagSelect.setOptions(options, steamTagSelect.getValue(), false);
      refreshFilterUi();
    },
    setCollectionSuggestions: (collections: string[]) => {
      const options: CustomSelectOption<string>[] = [
        { value: "", label: "All" },
        ...collections.map((collection) => ({ value: collection, label: collection })),
      ];
      collectionSelect.setOptions(options, collectionSelect.getValue(), false);
      refreshFilterUi();
    },
    setCollectionFilter: (collection: string, shouldNotify = true) => {
      const normalizedCollection = collection.trim();
      const nextValue = normalizedCollection.length > 0 ? normalizedCollection : DEFAULT_FILTERS.collection;
      return setSelectValue(collectionSelect, nextValue, shouldNotify);
    },
  };
};
