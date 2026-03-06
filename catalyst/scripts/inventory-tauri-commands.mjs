import { promises as fs } from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const RUST_SRC_ROOT = path.join(ROOT, 'src-tauri', 'src');
const LIB_RS = path.join(RUST_SRC_ROOT, 'lib.rs');
const OUTPUT = path.join(ROOT, 'docs', 'command-inventory.md');

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

function collectAnnotatedCommands(content, filePath) {
  const commands = [];
  const lines = content.split(/\r?\n/);
  let waitingForFn = false;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];

    if (line.includes('#[tauri::command]')) {
      waitingForFn = true;
      continue;
    }

    if (!waitingForFn) {
      continue;
    }

    const fnMatch = line.match(/^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(/);
    if (fnMatch) {
      commands.push({
        name: fnMatch[1],
        file: path.relative(ROOT, filePath).replaceAll(path.sep, '/'),
        line: index + 1,
      });
      waitingForFn = false;
      continue;
    }

    if (line.trim().length > 0 && !line.trim().startsWith('#')) {
      waitingForFn = false;
    }
  }

  return commands;
}

function collectRegisteredCommandsFromLibRs(content) {
  const handlerMatch = content.match(/generate_handler!\s*\[([\s\S]*?)\]/m);
  if (!handlerMatch) {
    return [];
  }

  return handlerMatch[1]
    .split(',')
    .map((item) => item.trim())
    .filter((item) => item.length > 0)
    .map((item) => item.replace(/\s+/g, ''));
}

function toSet(values) {
  return new Set(values);
}

function diff(a, b) {
  return [...a].filter((value) => !b.has(value)).sort((lhs, rhs) => lhs.localeCompare(rhs));
}

async function main() {
  const rustFiles = await walkRustFiles(RUST_SRC_ROOT);
  const allAnnotated = [];

  for (const filePath of rustFiles) {
    const content = await fs.readFile(filePath, 'utf8');
    allAnnotated.push(...collectAnnotatedCommands(content, filePath));
  }

  allAnnotated.sort((lhs, rhs) => lhs.name.localeCompare(rhs.name));

  const libRsContent = await fs.readFile(LIB_RS, 'utf8');
  const registeredCommands = collectRegisteredCommandsFromLibRs(libRsContent).sort((lhs, rhs) =>
    lhs.localeCompare(rhs)
  );

  const annotatedNames = allAnnotated.map((item) => item.name);
  const annotatedSet = toSet(annotatedNames);
  const registeredSet = toSet(registeredCommands);

  const annotatedNotRegistered = diff(annotatedSet, registeredSet);
  const registeredWithoutAttribute = diff(registeredSet, annotatedSet);

  const lines = [];
  lines.push('# Tauri Command Inventory');
  lines.push('');
  lines.push(`Generated: ${new Date().toISOString()}`);
  lines.push(`Annotated commands found: ${allAnnotated.length}`);
  lines.push(`Registered in generate_handler: ${registeredCommands.length}`);
  lines.push('');
  lines.push('## Registered commands');
  lines.push('');

  for (const command of registeredCommands) {
    const source = allAnnotated.find((item) => item.name === command);
    if (source) {
      lines.push(`- ${command} (${source.file}:${source.line})`);
    } else {
      lines.push(`- ${command} (source not found)`);
    }
  }

  lines.push('');
  lines.push('## Drift checks');
  lines.push('');

  if (annotatedNotRegistered.length === 0) {
    lines.push('- Annotated but not registered: none');
  } else {
    lines.push('- Annotated but not registered:');
    for (const command of annotatedNotRegistered) {
      lines.push(`  - ${command}`);
    }
  }

  if (registeredWithoutAttribute.length === 0) {
    lines.push('- Registered without #[tauri::command]: none');
  } else {
    lines.push('- Registered without #[tauri::command]:');
    for (const command of registeredWithoutAttribute) {
      lines.push(`  - ${command}`);
    }
  }

  lines.push('');
  lines.push('## Source coverage (annotated commands)');
  lines.push('');

  for (const item of allAnnotated) {
    lines.push(`- ${item.name} (${item.file}:${item.line})`);
  }

  await fs.mkdir(path.dirname(OUTPUT), { recursive: true });
  await fs.writeFile(OUTPUT, `${lines.join('\n')}\n`, 'utf8');
  console.log(`Wrote ${path.relative(ROOT, OUTPUT)} with ${allAnnotated.length} commands.`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});