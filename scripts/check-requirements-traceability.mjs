import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const requirementsPath = resolve(
  root,
  process.env.AGORA_REQUIREMENTS_PATH ?? "requirements.md",
);
const traceabilityPath = resolve(
  root,
  process.env.AGORA_TRACEABILITY_PATH ?? "docs/traceability.md",
);

function ids(path, pattern, label) {
  const text = readFileSync(path, "utf8");
  const values = [...text.matchAll(pattern)].map((match) => Number(match[1]));
  const unique = new Set(values);
  if (unique.size !== values.length) {
    throw new Error(`${label} contains a duplicate requirement identifier`);
  }
  if (values.length === 0) {
    throw new Error(`${label} contains no requirements`);
  }
  return [...unique].sort((left, right) => left - right);
}

const requirements = ids(requirementsPath, /^- \*\*R(\d+) —/gm, "requirements");
const traceability = ids(traceabilityPath, /^\| R(\d+) /gm, "traceability");
const maximum = requirements.at(-1);
const expected = Array.from({ length: maximum }, (_, index) => index + 1);
if (JSON.stringify(requirements) !== JSON.stringify(expected)) {
  throw new Error("requirements must form one contiguous R1..Rmax sequence");
}
if (JSON.stringify(traceability) !== JSON.stringify(requirements)) {
  const required = new Set(requirements);
  const traced = new Set(traceability);
  const missing = requirements.filter((id) => !traced.has(id));
  const stale = traceability.filter((id) => !required.has(id));
  throw new Error(
    `traceability mismatch; missing=${missing.join(",") || "none"}; stale=${stale.join(",") || "none"}`,
  );
}

console.log(`Agora requirements and traceability close exactly over R1..R${maximum}`);
