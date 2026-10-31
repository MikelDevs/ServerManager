import assert from "node:assert/strict";
import { test } from "node:test";
import { cleanupPlan, exitCode, MAINTENANCE_MARKER, runTauri } from "./tauri.mjs";

function fixture({ buildCode = 0, binary = MAINTENANCE_MARKER, report, cleanupCode = 0, platform = "darwin", env = {} } = {}) {
  const calls = [];
  const messages = [];
  const deps = {
    platform, env, projectRoot: "/project with spaces", nodeExecutable: "/node",
    resolveCli: () => "/official tauri/tauri.js",
    read: async () => Buffer.from(binary),
    log: (message) => messages.push(message),
    error: (message) => messages.push(message),
    run: async (program, args, options) => {
      calls.push({ program, args, options });
      if (program === "/node") return { code: buildCode, stdout: "", stderr: "" };
      if (args[0] === "--maintenance-version") return { code: 0, stdout: `${MAINTENANCE_MARKER}\n`, stderr: "" };
      return { code: cleanupCode, stdout: JSON.stringify(report ?? { supported: true, removed: ["/Volumes/dmg.old/Server Manager.app"], failures: [] }), stderr: "" };
    },
  };
  return { deps, calls, messages };
}

test("forwards all CLI arguments without a shell and cleans exactly the generated bundle", async () => {
  const { deps, calls } = fixture();
  assert.equal(await runTauri(["build", "--bundles", "dmg"], deps), 0);
  assert.deepEqual(calls[0], {
    program: "/node", args: ["/official tauri/tauri.js", "build", "--bundles", "dmg"],
    options: { cwd: "/project with spaces", env: {} },
  });
  assert.equal(calls[2].program, "/project with spaces/src-tauri/target/release/server-manager");
  assert.deepEqual(calls[2].args, [
    "--cleanup-app-registrations", "--build-bundle-path",
    "/project with spaces/src-tauri/target/release/bundle/macos/Server Manager.app", "--json",
  ]);
});

test("failed packaging still cleans with a compatible binary and preserves Tauri exit", async () => {
  const { deps, calls } = fixture({ buildCode: 42 });
  assert.equal(await runTauri(["build"], deps), 42);
  assert.equal(calls.length, 3);
});

test("never launches an old GUI binary after success or failure", async () => {
  for (const buildCode of [0, 7]) {
    const { deps, calls, messages } = fixture({ buildCode, binary: "old Server Manager 0.4.0" });
    assert.equal(await runTauri(["build"], deps), buildCode || 1);
    assert.equal(calls.length, 1);
    assert.ok(messages.some((line) => line.includes("no se abrió la aplicación anterior")));
  }
});

test("cleanup failure makes successful release fail while retaining an existing build failure", async () => {
  for (const buildCode of [0, 9]) {
    const { deps, messages } = fixture({ buildCode, cleanupCode: 1, report: { supported: true, removed: [], failures: [{ path: "/Volumes/dmg.old/Server Manager.app", message: "still registered" }] } });
    assert.equal(await runTauri(["build"], deps), buildCode || 1);
    assert.ok(messages.some((line) => line.includes("still registered")));
  }
});

test("development, help, non-bundling, and other platforms do not run maintenance", async () => {
  for (const [args, platform] of [
    [["dev"], "darwin"], [["build", "--help"], "darwin"],
    [["build", "--no-bundle"], "darwin"], [["build"], "linux"], [["build"], "win32"],
  ]) {
    const { deps, calls } = fixture({ platform });
    assert.equal(await runTauri(args, deps), 0);
    assert.equal(calls.length, 1);
  }
});

test("uses known macOS target and debug layouts without accepting arbitrary target paths", () => {
  const defaults = { platform: "darwin", projectRoot: "/project", env: {} };
  assert.deepEqual(cleanupPlan(["-v", "build", "--target=universal-apple-darwin", "--debug"], defaults), {
    executable: "/project/src-tauri/target/universal-apple-darwin/debug/server-manager",
    bundlePath: "/project/src-tauri/target/universal-apple-darwin/debug/bundle/macos/Server Manager.app",
  });
  assert.ok(cleanupPlan(["build", "--target", "../../Applications"], defaults).error);
  assert.ok(cleanupPlan(["build"], { ...defaults, env: { CARGO_TARGET_DIR: "/elsewhere" } }).error);
});

test("unsupported output layout never executes a guessed binary", async () => {
  const { deps, calls } = fixture({ env: { CARGO_TARGET_DIR: "/different-target" } });
  assert.equal(await runTauri(["build"], deps), 1);
  assert.equal(calls.length, 1);
});

test("preserves signal exit codes", () => {
  assert.equal(exitCode({ code: null, signal: "SIGINT" }), 130);
  assert.equal(exitCode({ code: null, signal: "SIGTERM" }), 143);
});
