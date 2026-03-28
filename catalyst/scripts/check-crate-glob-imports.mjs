import { promises as fs } from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const SERVICES_ROOT = path.join(ROOT, 'src-tauri', 'src', 'application', 'services');

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

function collectCrateGlobImports(content, filePath) {
  const lines = content.split(/\r?\n/);
  const entries = [];
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

    if (/^\s*use\s+[^;]*::\*\s*;\s*$/.test(line)) {
      const relativePath = path.relative(ROOT, filePath).replaceAll(path.sep, '/');
      entries.push(`${relativePath}:${index + 1}`);
    }
  }

  return entries;
}

async function main() {
  const rustFiles = await walkRustFiles(SERVICES_ROOT);
  const violations = [];

  for (const filePath of rustFiles) {
    const content = await fs.readFile(filePath, 'utf8');
    violations.push(...collectCrateGlobImports(content, filePath));
  }

  violations.sort((lhs, rhs) => lhs.localeCompare(rhs));
  if (violations.length > 0) {
    console.error('Architecture check failed: service modules must not use wildcard imports outside tests.');
    for (const entry of violations) {
      console.error(`- ${entry}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log('Architecture check passed: no wildcard imports found in services outside tests.');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
