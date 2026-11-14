import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const checker = resolve(root, "scripts/check-package-classification.mjs");
const baseline = JSON.parse(
  readFileSync(resolve(root, "package-classification.json"), "utf8"),
);
const temporary = mkdtempSync(resolve(tmpdir(), "agora-package-classification-"));

function clone() {
  return structuredClone(baseline);
}

function requireRefusal(name, mutate) {
  const candidate = clone();
  mutate(candidate);
  const path = resolve(temporary, `${name}.json`);
  writeFileSync(path, `${JSON.stringify(candidate, null, 2)}\n`, { mode: 0o600 });
  const result = spawnSync(process.execPath, [checker], {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, AGORA_PACKAGE_CLASSIFICATION_PATH: path },
  });
  if (result.status === 0) {
    throw new Error(`${name} mutant was accepted`);
  }
}

try {
  requireRefusal("missing", (value) => {
    delete value.packages["agora-engine"];
  });
  requireRefusal("stale", (value) => {
    value.packages["agora-stale"] = structuredClone(value.packages["agora-engine"]);
    value.packages["agora-stale"].path = "stale";
  });
  requireRefusal("path", (value) => {
    value.packages["agora-client"].path = "service/agora-service";
  });
  requireRefusal("category", (value) => {
    value.packages["agora-client"].category = "misc";
  });
  requireRefusal("category-path", (value) => {
    value.packages["agora-client"].category = "service";
  });
  requireRefusal("maturity", (value) => {
    value.packages["agora-client"].maturity = "done";
  });
  requireRefusal("effect", (value) => {
    value.packages["agora-client"].effect_class = "ambient";
  });
  requireRefusal("audience", (value) => {
    value.packages["agora-client"].audiences = ["everyone"];
  });
  requireRefusal("audience-order", (value) => {
    value.packages["agora-host"].audiences = ["trusted-host", "application"];
  });
  requireRefusal("publish", (value) => {
    value.packages["agora-client"].publish = true;
  });
  requireRefusal("extra-field", (value) => {
    value.packages["agora-client"].note = "unbounded";
  });
} finally {
  rmSync(temporary, { recursive: true, force: true });
}

console.log("Agora package classification rejects inventory and vocabulary mutants");
