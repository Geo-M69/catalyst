import { promises as fs } from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const SERVICES_ROOT = path.join(ROOT, 'src-tauri', 'src', 'application', 'services');

const LEGACY_EXCEPTIONS = new Set([]);

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

function collectViolations(content, filePath) {
  const relativePath = path.relative(ROOT, filePath).replaceAll(path.sep, '/');
  if (LEGACY_EXCEPTIONS.has(relativePath)) {
    return [];
  }

  const lines = content.split(/\r?\n/);
  const violations = [];

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];

    const hasDirectCrateListImport = /\buse\s+crate\s*::\s*\{/.test(line);
    const hasDirectCrateCall = /\bcrate::[a-z_][a-z0-9_]*\s*\(/.test(line);

    if (hasDirectCrateListImport || hasDirectCrateCall) {
      violations.push(`${relativePath}:${index + 1}`);
    }
  }

  return violations;
}

async function main() {
  const rustFiles = await walkRustFiles(SERVICES_ROOT);
  const violations = [];

  for (const filePath of rustFiles) {
    const content = await fs.readFile(filePath, 'utf8');
    violations.push(...collectViolations(content, filePath));
  }

  violations.sort((left, right) => left.localeCompare(right));

  if (violations.length > 0) {
    console.error('Architecture check failed: non-legacy services must not call runtime helpers directly from crate root.');
    console.error('Legacy exceptions are explicitly tracked in scripts/check-service-runtime-imports.mjs.');
    for (const violation of violations) {
      console.error(`- ${violation}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log('Architecture check passed: non-legacy service modules avoid direct runtime helper imports.');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
