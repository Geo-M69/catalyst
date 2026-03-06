const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"];

export const formatBytes = (sizeInBytes?: number | null): string | null => {
  if (typeof sizeInBytes !== "number" || !Number.isFinite(sizeInBytes) || sizeInBytes <= 0) {
    return null;
  }

  let unitIndex = 0;
  let value = sizeInBytes;
  while (value >= 1024 && unitIndex < BYTE_UNITS.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const fractionDigits = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(fractionDigits)} ${BYTE_UNITS[unitIndex]}`;
};

export const isFiniteNonNegativeNumber = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v) && v >= 0;
