import { promises as fs } from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();

const FILE_LIMITS = [
  { path: 'src/mainPage/mainPage.ts', maxLines: 3900 },
  { path: 'src/mainPage/components/gamePropertiesPanel.ts', maxLines: 3400 },
  // Transitional backend adapters: keep strict no-growth caps while they are decomposed.
  { path: 'src-tauri/src/lib_runtime_impl.rs', maxLines: 6700 },
  { path: 'src-tauri/src/infrastructure/library_port.rs', maxLines: 2900 },
  { path: 'src-tauri/src/application/services/library_service.rs', maxLines: 3000 },
  { path: 'src-tauri/src/infrastructure/library_steam_review.rs', maxLines: 1200 },
];

async function countLines(filePath) {
  const content = await fs.readFile(filePath, 'utf8');
  return content.split(/\r?\n/).length;
}

async function main() {
  const violations = [];

  for (const rule of FILE_LIMITS) {
    const absolutePath = path.join(ROOT, rule.path);
    const lineCount = await countLines(absolutePath);
    if (lineCount > rule.maxLines) {
      violations.push(`${rule.path}: ${lineCount} lines (limit ${rule.maxLines})`);
    }
  }

  if (violations.length > 0) {
    console.error('Architecture check failed: file-size guardrail exceeded.');
    for (const violation of violations) {
      console.error(`- ${violation}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log('Architecture check passed: max file-size guardrails are within limits.');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
