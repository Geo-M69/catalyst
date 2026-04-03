import { promises as fs } from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const APPLICATION_ROOT = path.join(ROOT, 'src-tauri', 'src', 'application');

const LEGACY_EXCEPTIONS = new Set([
  'src-tauri/src/application/bootstrap.rs',
  'src-tauri/src/application/canonicalizer.rs',
]);

const ALLOWED_CALL_PREFIXES = [
  'crate::application::',
  'crate::domain::',
];

async function walkRustFiles(dir) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        return walkRustFiles(fullPath);
      }
      if (entry.isFile() && fullPath.endsWith('.rs')) {
        return [fullPath];
      }
      return [];
    })
  );

  return files.flat();
}

function isAllowedCallPath(pathValue) {
  return ALLOWED_CALL_PREFIXES.some((prefix) => pathValue.startsWith(prefix));
}

function collectViolations(content, filePath) {
  const relativePath = path.relative(ROOT, filePath).replaceAll(path.sep, '/');
  if (LEGACY_EXCEPTIONS.has(relativePath)) {
    return [];
  }

  const lines = content.split(/\r?\n/);
  const violations = [];
  let pendingTestModule = false;
  let inTestModule = false;
  let testBraceDepth = 0;

  const openBraceCount = (line) => (line.match(/\{/g) ?? []).length;
  const closeBraceCount = (line) => (line.match(/\}/g) ?? []).length;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];

    if (!inTestModule && /^\s*#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*$/.test(line)) {
      pendingTestModule = true;
      continue;
    }

    if (!inTestModule && pendingTestModule && /^\s*mod\s+[a-zA-Z_][a-zA-Z0-9_]*\s*\{/.test(line)) {
      inTestModule = true;
      pendingTestModule = false;
      testBraceDepth = openBraceCount(line) - closeBraceCount(line);
      continue;
    }

    if (inTestModule) {
      testBraceDepth += openBraceCount(line) - closeBraceCount(line);
      if (testBraceDepth <= 0) {
        inTestModule = false;
      }
      continue;
    }

    const trimmed = line.trim();
    if (trimmed.startsWith('use crate::')) {
      if (
        trimmed.startsWith('use crate::{') ||
        (!trimmed.startsWith('use crate::application::') &&
          !trimmed.startsWith('use crate::domain::'))
      ) {
        violations.push(`${relativePath}:${index + 1}`);
        continue;
      }
    }

    const callMatch = line.match(/\b(crate::[a-z_][a-z0-9_:]*)\s*\(/);
    if (callMatch && !isAllowedCallPath(callMatch[1])) {
      violations.push(`${relativePath}:${index + 1}`);
      continue;
    }
  }

  return violations;
}

async function main() {
  const rustFiles = await walkRustFiles(APPLICATION_ROOT);
  const violations = [];

  for (const filePath of rustFiles) {
    const content = await fs.readFile(filePath, 'utf8');
    violations.push(...collectViolations(content, filePath));
  }

  violations.sort((left, right) => left.localeCompare(right));

  if (violations.length > 0) {
    console.error('Architecture check failed: application modules must not import or call runtime helpers from crate root.');
    console.error('Only crate::application::* and crate::domain::* references are allowed in non-legacy files.');
    console.error('Legacy exceptions are explicitly tracked in scripts/check-service-runtime-imports.mjs.');
    for (const violation of violations) {
      console.error(`- ${violation}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log('Architecture check passed: non-legacy application modules avoid crate-root runtime imports and calls.');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
