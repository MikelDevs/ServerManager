#!/usr/bin/env node
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { constants } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const PROJECT_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const MAINTENANCE_MARKER = "SERVER_MANAGER_MAINTENANCE_CLI_V1";
const MAC_TARGETS = new Set([
  "aarch64-apple-darwin", "x86_64-apple-darwin", "universal-apple-darwin",
]);

export function exitCode(result) {
  if (result.signal) return 128 + (constants.signals[result.signal] ?? 1);
  return result.code ?? 1;
}

function option(args, long, short) {
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === long || (short && args[index] === short)) return args[index + 1];
    if (args[index].startsWith(`${long}=`)) return args[index].slice(long.length + 1);
  }
  return undefined;
}

export function cleanupPlan(args, { platform, projectRoot, env }) {
  const buildIndex = args.findIndex((arg) => !/^-v+$/.test(arg) && arg !== "--verbose");
  if (platform !== "darwin" || args[buildIndex] !== "build") return null;
  const commandArgs = args.slice(buildIndex + 1);
  const separator = commandArgs.indexOf("--");
  const flags = separator < 0 ? commandArgs : commandArgs.slice(0, separator);
  if (flags.some((arg) => ["-h", "--help", "--version", "-V", "--no-bundle"].includes(arg))) {
    return null;
  }
  const targetRoot = path.join(projectRoot, "src-tauri", "target");
  if (env.CARGO_TARGET_DIR && path.resolve(projectRoot, "src-tauri", env.CARGO_TARGET_DIR) !== targetRoot) {
    return { error: "La limpieza automática requiere src-tauri/target; CARGO_TARGET_DIR personalizado no está soportado." };
  }
  const target = option(flags, "--target", "-t") || env.CARGO_BUILD_TARGET;
  if (target && !MAC_TARGETS.has(target)) {
    return { error: `La limpieza automática no reconoce el target ${target}; no se ejecutará ningún binario de esa ruta.` };
  }
  const profile = flags.includes("--debug") || flags.includes("-d") ? "debug" : "release";
  const output = target ? path.join(targetRoot, target, profile) : path.join(targetRoot, profile);
  return {
    executable: path.join(output, "server-manager"),
    bundlePath: path.join(output, "bundle", "macos", "Server Manager.app"),
  };
}

export function runCommand(program, args, { cwd, env, capture = false, timeoutMs } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(program, args, { cwd, env, stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit" });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    const forwards = new Map(["SIGINT", "SIGTERM"].map((signal) => [signal, () => child.kill(signal)]));
    for (const [signal, handler] of forwards) process.on(signal, handler);
    const collect = (stream, chunk) => {
      // Maintenance JSON is small; bound diagnostics if a failing subprocess is noisy.
      if (stream === "stdout") stdout = (stdout + chunk).slice(-1_048_576);
      else stderr = (stderr + chunk).slice(-1_048_576);
    };
    child.stdout?.on("data", (chunk) => collect("stdout", chunk));
    child.stderr?.on("data", (chunk) => collect("stderr", chunk));
    const timer = timeoutMs ? setTimeout(() => { timedOut = true; child.kill("SIGKILL"); }, timeoutMs) : null;
    const cleanup = () => {
      if (timer) clearTimeout(timer);
      for (const [signal, handler] of forwards) process.off(signal, handler);
    };
    child.once("error", (error) => { cleanup(); reject(error); });
    child.once("close", (code, signal) => {
      cleanup();
      if (timedOut) reject(new Error("La limpieza de registros excedió el tiempo de espera."));
      else resolve({ code, signal, stdout, stderr });
    });
  });
}

async function cleanRegistrations(plan, dependencies) {
  const { read, run, log, error, projectRoot, env } = dependencies;
  if (plan.error) throw new Error(plan.error);
  // Never execute a 0.4.0 binary: unknown CLI flags would open its GUI and load real projects.
  const executableBytes = await read(plan.executable);
  if (!executableBytes.includes(Buffer.from(MAINTENANCE_MARKER))) {
    throw new Error("No hay un binario compatible con la limpieza de registros; no se abrió la aplicación anterior.");
  }
  const options = { cwd: projectRoot, env, capture: true };
  const capability = await run(plan.executable, ["--maintenance-version"], { ...options, timeoutMs: 10_000 });
  if (exitCode(capability) !== 0 || capability.stdout.trim() !== MAINTENANCE_MARKER) {
    throw new Error("El binario no confirmó el modo de mantenimiento esperado.");
  }
  const result = await run(plan.executable, [
    "--cleanup-app-registrations", "--build-bundle-path", plan.bundlePath, "--json",
  ], { ...options, timeoutMs: 75_000 });
  let report;
  try { report = JSON.parse(result.stdout); }
  catch { throw new Error(result.stderr.trim() || "El mantenimiento no devolvió un resultado válido."); }
  if (!report.supported || !Array.isArray(report.removed) || !Array.isArray(report.failures)) {
    throw new Error("El resultado del mantenimiento no tiene el formato esperado.");
  }
  log(`Limpieza de macOS: ${report.removed.length} referencias antiguas de Server Manager retiradas.`);
  for (const failure of report.failures) error(`${failure.path}: ${failure.message}`);
  if (report.failures.length > 0 || exitCode(result) !== 0) {
    throw new Error(result.stderr.trim() || "No se pudo completar la limpieza de referencias de Server Manager.");
  }
}

export async function runTauri(args, overrides = {}) {
  const dependencies = {
    platform: process.platform,
    projectRoot: PROJECT_ROOT,
    env: process.env,
    nodeExecutable: process.execPath,
    resolveCli: () => require.resolve("@tauri-apps/cli/tauri.js"),
    read: readFile,
    run: runCommand,
    log: console.log,
    error: console.error,
    ...overrides,
  };
  const { projectRoot, env, run, error } = dependencies;
  const plan = cleanupPlan(args, dependencies);
  let buildCode = 1;
  try {
    const result = await run(dependencies.nodeExecutable, [dependencies.resolveCli(), ...args], {
      cwd: projectRoot, env,
    });
    buildCode = exitCode(result);
  } catch (caught) {
    error(`No se pudo ejecutar Tauri: ${caught.message}`);
  }
  if (plan) {
    try {
      // Also clean after interrupted/failed packaging when a compatible previous binary exists.
      await cleanRegistrations(plan, dependencies);
    } catch (caught) {
      error(`Limpieza pendiente: ${caught.message}`);
      // Keep Tauri's original error or signal. Successful packaging must not conceal failed cleanup.
      if (buildCode === 0) buildCode = 1;
    }
  }
  return buildCode;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = await runTauri(process.argv.slice(2));
}
