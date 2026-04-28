const RECENTLY_PLAYED_PERF_STORAGE_KEY = "catalyst.debug.recentlyPlayedPerf";
const SLOW_FRAME_THRESHOLD_MS = 18;
const VERY_SLOW_FRAME_THRESHOLD_MS = 33;

type RecentlyPlayedPerfEndReason = "settled" | "stopped" | "reduced-motion" | "tap";

type RecentlyPlayedPerfState = {
  active: boolean;
  sessionLabel: string;
  startedAtMs: number;
  frameCount: number;
  totalFrameDeltaMs: number;
  maxFrameDeltaMs: number;
  slowFrameCount: number;
  verySlowFrameCount: number;
  longTaskCount: number;
  longTaskTotalMs: number;
};

const createInitialState = (): RecentlyPlayedPerfState => ({
  active: false,
  sessionLabel: "",
  startedAtMs: 0,
  frameCount: 0,
  totalFrameDeltaMs: 0,
  maxFrameDeltaMs: 0,
  slowFrameCount: 0,
  verySlowFrameCount: 0,
  longTaskCount: 0,
  longTaskTotalMs: 0,
});

const parseEnabledFlag = (rawValue: string | null): boolean => {
  if (!rawValue) {
    return false;
  }

  const normalized = rawValue.trim().toLocaleLowerCase();
  return normalized === "1" || normalized === "true" || normalized === "yes" || normalized === "on";
};

const resolveRecentlyPlayedPerfEnabled = (): boolean => {
  try {
    if (parseEnabledFlag(localStorage.getItem(RECENTLY_PLAYED_PERF_STORAGE_KEY))) {
      return true;
    }
  } catch {
    // Ignore storage restrictions.
  }

  try {
    const query = new URLSearchParams(window.location.search);
    return parseEnabledFlag(query.get("recentlyPlayedPerf"));
  } catch {
    return false;
  }
};

export const createRecentlyPlayedPerfMonitor = () => {
  const enabled = resolveRecentlyPlayedPerfEnabled();
  const state = createInitialState();
  let longTaskObserver: PerformanceObserver | null = null;

  const resetState = (): void => {
    const next = createInitialState();
    state.active = next.active;
    state.sessionLabel = next.sessionLabel;
    state.startedAtMs = next.startedAtMs;
    state.frameCount = next.frameCount;
    state.totalFrameDeltaMs = next.totalFrameDeltaMs;
    state.maxFrameDeltaMs = next.maxFrameDeltaMs;
    state.slowFrameCount = next.slowFrameCount;
    state.verySlowFrameCount = next.verySlowFrameCount;
    state.longTaskCount = next.longTaskCount;
    state.longTaskTotalMs = next.longTaskTotalMs;
  };

  const ensureLongTaskObserver = (): void => {
    if (!enabled || longTaskObserver !== null || typeof PerformanceObserver === "undefined") {
      return;
    }

    const supportedEntryTypes = PerformanceObserver.supportedEntryTypes ?? [];
    if (!supportedEntryTypes.includes("longtask")) {
      return;
    }

    try {
      longTaskObserver = new PerformanceObserver((entryList) => {
        if (!state.active) {
          return;
        }

        for (const entry of entryList.getEntries()) {
          state.longTaskCount += 1;
          state.longTaskTotalMs += entry.duration;
        }
      });
      longTaskObserver.observe({ type: "longtask", buffered: true });
    } catch {
      longTaskObserver = null;
    }
  };

  const startSession = (sessionLabel: string): void => {
    if (!enabled || state.active) {
      return;
    }

    resetState();
    state.active = true;
    state.sessionLabel = sessionLabel;
    state.startedAtMs = performance.now();
    ensureLongTaskObserver();
  };

  const recordFrame = (deltaMs: number): void => {
    if (!enabled || !state.active || !Number.isFinite(deltaMs) || deltaMs <= 0) {
      return;
    }

    state.frameCount += 1;
    state.totalFrameDeltaMs += deltaMs;
    state.maxFrameDeltaMs = Math.max(state.maxFrameDeltaMs, deltaMs);

    if (deltaMs >= SLOW_FRAME_THRESHOLD_MS) {
      state.slowFrameCount += 1;
    }
    if (deltaMs >= VERY_SLOW_FRAME_THRESHOLD_MS) {
      state.verySlowFrameCount += 1;
    }
  };

  const endSession = (reason: RecentlyPlayedPerfEndReason): void => {
    if (!enabled || !state.active) {
      return;
    }

    const elapsedMs = Math.max(0, performance.now() - state.startedAtMs);
    const averageDeltaMs = state.frameCount > 0 ? state.totalFrameDeltaMs / state.frameCount : 0;
    const approximateFps = averageDeltaMs > 0 ? 1000 / averageDeltaMs : 0;

    console.debug(
      `[recently-played][perf] ${state.sessionLabel} -> ${reason}`,
      {
        elapsedMs: Number(elapsedMs.toFixed(1)),
        frameCount: state.frameCount,
        averageDeltaMs: Number(averageDeltaMs.toFixed(2)),
        maxDeltaMs: Number(state.maxFrameDeltaMs.toFixed(2)),
        slowFrameCount: state.slowFrameCount,
        verySlowFrameCount: state.verySlowFrameCount,
        approximateFps: Number(approximateFps.toFixed(1)),
        longTaskCount: state.longTaskCount,
        longTaskTotalMs: Number(state.longTaskTotalMs.toFixed(1)),
      }
    );

    resetState();
  };

  return {
    enabled,
    startSession,
    recordFrame,
    endSession,
  };
};
