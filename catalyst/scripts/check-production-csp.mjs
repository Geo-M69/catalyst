import { promises as fs } from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const TAURI_CONFIG_PATH = path.join(ROOT, 'src-tauri', 'tauri.conf.json');
const FORBIDDEN_TOKENS = [
  'http://localhost',
  'https://localhost',
  'ws://localhost',
  'wss://localhost',
  '127.0.0.1',
];

async function main() {
  const content = await fs.readFile(TAURI_CONFIG_PATH, 'utf8');
  const parsed = JSON.parse(content);
  const csp = parsed?.app?.security?.csp;

  if (typeof csp !== 'string' || csp.trim().length === 0) {
    console.error('Architecture check failed: src-tauri/tauri.conf.json app.security.csp must be a non-empty string.');
    process.exitCode = 1;
    return;
  }

  const normalized = csp.toLowerCase();
  const violations = FORBIDDEN_TOKENS.filter((token) => normalized.includes(token));

  if (violations.length > 0) {
    console.error('Architecture check failed: production CSP must not contain localhost dev origins.');
    for (const violation of violations) {
      console.error(`- src-tauri/tauri.conf.json csp contains forbidden token: ${violation}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log('Architecture check passed: production CSP has no localhost dev origins.');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
