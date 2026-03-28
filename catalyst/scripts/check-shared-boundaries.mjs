import { promises as fs } from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const SHARED_ROOT = path.join(ROOT, 'src', 'shared');

async function walkTsFiles(dir) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        return walkTsFiles(fullPath);
      }
      if (entry.isFile() && fullPath.endsWith('.ts')) {
        return [fullPath];
      }
      return [];
    })
  );

  return files.flat();
}

function collectViolations(content, filePath) {
  const relativePath = path.relative(ROOT, filePath).replaceAll(path.sep, '/');
  const lines = content.split(/\r?\n/);
  const violations = [];

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (!/^\s*import\s+/.test(line)) {
      continue;
    }

    if (/mainPage\//.test(line)) {
      violations.push(`${relativePath}:${index + 1}`);
    }
  }

  return violations;
}

async function main() {
  const files = await walkTsFiles(SHARED_ROOT);
  const violations = [];

  for (const filePath of files) {
    const content = await fs.readFile(filePath, 'utf8');
    violations.push(...collectViolations(content, filePath));
  }

  violations.sort((left, right) => left.localeCompare(right));

  if (violations.length > 0) {
    console.error('Architecture check failed: src/shared must not import from src/mainPage.');
    for (const violation of violations) {
      console.error(`- ${violation}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log('Architecture check passed: src/shared has no imports from src/mainPage.');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
