import { promises as fs } from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const RUST_ROOT = path.join(ROOT, 'src-tauri', 'src');
const FORBIDDEN_COMMANDS = new Set(['bash', 'sh', 'openssl', 'steamcmd']);
const COMMAND_NEW_PATTERN = /\b(?:std::process::)?Command::new\(\s*"([^"]+)"\s*\)/g;

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

function lineNumberAtIndex(content, index) {
  return content.slice(0, index).split(/\r?\n/).length;
}

function collectViolations(content, filePath) {
  const violations = [];
  const relativePath = path.relative(ROOT, filePath).replaceAll(path.sep, '/');

  for (const match of content.matchAll(COMMAND_NEW_PATTERN)) {
    const command = match[1]?.trim().toLowerCase();
    if (!command || !FORBIDDEN_COMMANDS.has(command)) {
      continue;
    }

    const lineNumber = lineNumberAtIndex(content, match.index ?? 0);
    violations.push(`${relativePath}:${lineNumber} (${command})`);
  }

  return violations;
}

async function main() {
  const files = await walkRustFiles(RUST_ROOT);
  const violations = [];

  for (const filePath of files) {
    const content = await fs.readFile(filePath, 'utf8');
    violations.push(...collectViolations(content, filePath));
  }

  violations.sort((left, right) => left.localeCompare(right));

  if (violations.length > 0) {
    console.error('Architecture check failed: shell-based command paths are forbidden in Rust runtime code.');
    console.error('Use Rust-native implementations instead of bash/sh/openssl/steamcmd invocation.');
    for (const violation of violations) {
      console.error(`- ${violation}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log('Architecture check passed: no forbidden shell-based command invocations found in Rust sources.');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
