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

interface CustomSelectController<T extends string> extends BaseCustomSelectController {
  getValue: () => T;
  setValue: (value: T, notifyChange?: boolean) => void;
}

const FILTER_TEMPLATE = `
  <form id="library-filter-form" class="filter-form filter-form-panel" autocomplete="off">
    <div class="filter-field">
      <label class="field-label filter-field-label" for="filter-search">Search</label>
      <input id="filter-search" class="text-input filter-input filter-input-search" type="search" placeholder="Search games..." />
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
  const optionNodes = Array.from(field.querySelectorAll(".filter-select-option"));
  const areOptionButtons = optionNodes.every((optionNode) => optionNode instanceof HTMLButtonElement);

  if (
    !(trigger instanceof HTMLButtonElement)
    || !(triggerText instanceof HTMLElement)
    || !(menu instanceof HTMLElement)
    || !areOptionButtons
  ) {
    throw new Error("Custom select is missing required DOM elements");
  }

  const optionButtons = optionNodes as HTMLButtonElement[];
  let currentValue = "";

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
    const selectedOption = optionButtons.find((optionButton) => optionButton.classList.contains("is-selected")) ?? optionButtons[0];
    selectedOption.focus();
  };

  const setValue = (value: T, notifyChange = true): void => {
    const selectedOption = optionButtons.find((optionButton) => optionButton.dataset.value === value);
    if (!selectedOption) {
      return;
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
  };

  for (const optionButton of optionButtons) {
    optionButton.addEventListener("click", () => {
      const optionValue = optionButton.dataset.value;
      if (!optionValue) {
        return;
      }

      setValue(optionValue as T);
      closeMenu();
      trigger.focus();
    });
  }

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

  const initialValue = optionButtons[0]?.dataset.value;
  if (!initialValue) {
    throw new Error("Custom select must include at least one option with a value");
  }

  setValue(initialValue as T, false);

  return {
    getValue: (): T => currentValue as T,
    setValue,
    closeMenu,
  };
};

export interface FilterPanelController {
  getFilters: () => LibraryFilters;
}

export const createFilterPanel = (
  container: HTMLElement,
  onChange: () => void
): FilterPanelController => {
  container.innerHTML = FILTER_TEMPLATE;

  const searchInput = container.querySelector("#filter-search");
  const filterByField = container.querySelector("#filter-by-field");
  const platformField = container.querySelector("#platform-field");
  const sourceField = container.querySelector("#source-field");
  const kindField = container.querySelector("#kind-field");
  const genreField = container.querySelector("#genre-field");
  const sortByField = container.querySelector("#sort-by-field");
  const clearButton = container.querySelector("#clear-filters-button");

  if (
    !(searchInput instanceof HTMLInputElement)
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

  const notifyChange = (): void => {
    onChange();
  };

  const selectControllers: BaseCustomSelectController[] = [];
  const closeAllSelectMenus = (): void => {
    for (const selectController of selectControllers) {
      selectController.closeMenu();
    }
  };

  const filterBySelect = createCustomSelect<FilterByOption>(filterByField, notifyChange, closeAllSelectMenus);
  const platformSelect = createCustomSelect<PlatformFilter>(platformField, notifyChange, closeAllSelectMenus);
  const sourceSelect = createCustomSelect<SourceFilter>(sourceField, notifyChange, closeAllSelectMenus);
  const kindSelect = createCustomSelect<GameKindFilter>(kindField, notifyChange, closeAllSelectMenus);
  const genreSelect = createCustomSelect<GenreFilter>(genreField, notifyChange, closeAllSelectMenus);
  const sortBySelect = createCustomSelect<SortOption>(sortByField, notifyChange, closeAllSelectMenus);

  selectControllers.push(filterBySelect, platformSelect, sourceSelect, kindSelect, genreSelect, sortBySelect);

  const buildFiltersFromForm = (): LibraryFilters => {
    return {
      search: searchInput.value,
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
  };
};
