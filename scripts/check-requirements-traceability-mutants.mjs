import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const checker = resolve(root, "scripts/check-requirements-traceability.mjs");
const requirements = readFileSync(resolve(root, "requirements.md"), "utf8");
const traceability = readFileSync(resolve(root, "docs/traceability.md"), "utf8");
const temporary = mkdtempSync(resolve(tmpdir(), "agora-traceability-"));

function refuse(name, requiredText, tracedText) {
  const requiredPath = resolve(temporary, `${name}-requirements.md`);
  const tracedPath = resolve(temporary, `${name}-traceability.md`);
  writeFileSync(requiredPath, requiredText, { mode: 0o600 });
  writeFileSync(tracedPath, tracedText, { mode: 0o600 });
  const result = spawnSync(process.execPath, [checker], {
    cwd: root,
    encoding: "utf8",
    env: {
      ...process.env,
      AGORA_REQUIREMENTS_PATH: requiredPath,
      AGORA_TRACEABILITY_PATH: tracedPath,
    },
  });
  if (result.status === 0) {
    throw new Error(`${name} traceability mutant was accepted`);
  }
}

try {
  refuse("missing", requirements, traceability.replace(/^\| R92 .*\n/m, ""));
  refuse(
    "stale",
    requirements,
    `${traceability}\n| R999 stale | stale | stale | stale |\n`,
  );
  refuse(
    "requirement-gap",
    requirements.replace(/^- \*\*R63 .*?(?=^- \*\*R64 )/ms, ""),
    traceability,
  );
  refuse(
    "duplicate",
    `${requirements}\n- **R92 — duplicate:** duplicate.\n`,
    traceability,
  );
} finally {
  rmSync(temporary, { recursive: true, force: true });
}

console.log("Agora requirements traceability rejects missing, stale, gap, and duplicate mutants");
