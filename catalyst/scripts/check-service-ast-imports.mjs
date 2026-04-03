import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ROOT = path.resolve(__dirname, '..');
const MANIFEST_PATH = path.join(ROOT, 'tools', 'architecture-gates', 'Cargo.toml');
const CONFIG_PATH = path.join(ROOT, 'scripts', 'architecture-gates.json');

function resolveMode(argv) {
  const modeIndex = argv.findIndex((arg) => arg === '--mode');
  if (modeIndex >= 0 && modeIndex + 1 < argv.length) {
    return argv[modeIndex + 1];
  }
  return 'report';
}

function main() {
  const mode = resolveMode(process.argv.slice(2));
  const args = [
    'run',
    '--quiet',
    '--manifest-path',
    MANIFEST_PATH,
    '--',
    '--root',
    ROOT,
    '--config',
    CONFIG_PATH,
    '--mode',
    mode,
  ];

  const result = spawnSync('cargo', args, {
    cwd: ROOT,
    stdio: 'inherit',
  });

  if (typeof result.status === 'number') {
    process.exit(result.status);
  }

  process.exit(1);
}

main();
