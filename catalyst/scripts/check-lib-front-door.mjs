import { promises as fs } from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const LIB_RS_PATH = path.join(ROOT, 'src-tauri', 'src', 'lib.rs');

const ALLOWED_LINE_PATTERNS = [
  /^mod\s+[a-z_][a-z0-9_]*\s*;\s*$/,
  /^pub\(crate\)\s+use\s+application::bootstrap::AppState\s*;\s*$/,
  /^include!\("lib_runtime_impl\.rs"\)\s*;\s*$/,
];

const REQUIRED_LINES = [
  'include!("lib_runtime_impl.rs");',
  'pub(crate) use application::bootstrap::AppState;',
];

function isAllowedLine(line) {
  if (line.trim().length === 0) {
    return true;
  }
  if (line.trim().startsWith('//')) {
    return true;
  }

  return ALLOWED_LINE_PATTERNS.some((pattern) => pattern.test(line));
}

async function main() {
  const content = await fs.readFile(LIB_RS_PATH, 'utf8');
  const lines = content.split(/\r?\n/);
  const violations = [];

  for (let index = 0; index < lines.length; index += 1) {
    if (!isAllowedLine(lines[index])) {
      violations.push(`src-tauri/src/lib.rs:${index + 1}`);
    }
  }

  for (const requiredLine of REQUIRED_LINES) {
    if (!content.includes(requiredLine)) {
      violations.push(`missing:${requiredLine}`);
    }
  }

  if (violations.length > 0) {
    console.error(
      'Architecture check failed: src-tauri/src/lib.rs must stay a front-door file (composition + exports only).'
    );
    for (const violation of violations) {
      console.error(`- ${violation}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log('Architecture check passed: src-tauri/src/lib.rs is front-door only.');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
