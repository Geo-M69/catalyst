import type { AppErrorPayload, AppErrorKind } from "../ipc/contracts";

const APP_ERROR_KINDS: readonly AppErrorKind[] = [
  "validation",
  "unauthorized",
  "not_found",
  "conflict",
  "external",
  "internal",
];

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null;

const hasNonEmptyMessage = (value: unknown): value is string =>
  typeof value === "string" && value.trim().length > 0;

const isAppErrorKind = (value: unknown): value is AppErrorKind =>
  typeof value === "string" && APP_ERROR_KINDS.includes(value as AppErrorKind);

export const isAppErrorPayload = (value: unknown): value is AppErrorPayload => {
  if (!isRecord(value)) {
    return false;
  }

  return (
    isAppErrorKind(value.kind) &&
    typeof value.code === "string" &&
    hasNonEmptyMessage(value.message)
  );
};

const tryParseJson = (value: string): unknown => {
  try {
    return JSON.parse(value);
  } catch {
    return undefined;
  }
};

const parseNestedPayload = (value: unknown): AppErrorPayload | null => {
  if (isAppErrorPayload(value)) {
    return value;
  }

  if (!isRecord(value)) {
    return null;
  }

  const candidateKeys: Array<keyof typeof value> = ["error", "payload", "cause", "data"];
  for (const key of candidateKeys) {
    const candidate = value[key];
    if (isAppErrorPayload(candidate)) {
      return candidate;
    }
    if (typeof candidate === "string") {
      const parsed = tryParseJson(candidate);
      if (isAppErrorPayload(parsed)) {
        return parsed;
      }
    }
  }

  return null;
};

export const normalizeAppError = (error: unknown, fallbackMessage: string): AppErrorPayload => {
  const nested = parseNestedPayload(error);
  if (nested) {
    return nested;
  }

  if (typeof error === "string") {
    const parsed = tryParseJson(error);
    if (isAppErrorPayload(parsed)) {
      return parsed;
    }
    if (hasNonEmptyMessage(error)) {
      return {
        kind: "internal",
        code: "legacy_error",
        message: error,
      };
    }
  }

  if (error instanceof Error && hasNonEmptyMessage(error.message)) {
    const parsed = tryParseJson(error.message);
    if (isAppErrorPayload(parsed)) {
      return parsed;
    }

    return {
      kind: "internal",
      code: "exception_error",
      message: error.message,
    };
  }

  return {
    kind: "internal",
    code: "unknown_error",
    message: fallbackMessage,
  };
};

export class IpcError extends Error {
  readonly appError: AppErrorPayload;
  readonly cause?: unknown;

  constructor(appError: AppErrorPayload, options?: { cause?: unknown }) {
    super(appError.message);
    this.name = "IpcError";
    this.appError = appError;
    if (options && "cause" in options) {
      this.cause = options.cause;
    }
  }
}

export const toErrorMessage = (error: unknown, fallbackMessage: string): string =>
  normalizeAppError(error, fallbackMessage).message;
