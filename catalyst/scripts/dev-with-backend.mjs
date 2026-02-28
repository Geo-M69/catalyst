import { spawn } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const npmCommand = process.platform === "win32" ? "npm.cmd" : "npm";
const rootDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const backendDir = resolve(rootDir, "backend");

const processes = [];
let isShuttingDown = false;

const spawnProcess = (name, cwd, args) => {
  const child = spawn(npmCommand, args, {
    cwd,
    stdio: "inherit",
    env: process.env
  });

  child.on("error", (error) => {
    console.error(`[dev-with-backend] Failed to start ${name}:`, error);
    shutdown(1);
  });

  child.on("exit", (code, signal) => {
    if (isShuttingDown) {
      return;
    }

    const exitCode = code ?? (signal ? 1 : 0);
    console.error(`[dev-with-backend] ${name} exited (code=${exitCode}, signal=${signal ?? "none"}).`);
    shutdown(exitCode);
  });

  processes.push(child);
};

const stopChild = (child) => {
  if (child.killed || child.exitCode !== null) {
    return;
  }

  if (process.platform === "win32") {
    spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
      stdio: "ignore"
    });
    return;
  }

  child.kill("SIGTERM");
};

const shutdown = (exitCode = 0) => {
  if (isShuttingDown) {
    return;
  }

  isShuttingDown = true;
  for (const child of processes) {
    stopChild(child);
  }

  setTimeout(() => {
    process.exit(exitCode);
  }, 200);
};

process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));

spawnProcess("frontend", rootDir, ["run", "dev"]);
spawnProcess("backend", backendDir, ["run", "dev"]);
