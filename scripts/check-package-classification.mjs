import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const classificationPath = resolve(
  root,
  process.env.AGORA_PACKAGE_CLASSIFICATION_PATH ?? "package-classification.json",
);

const allowedCategories = new Set([
  "adapter",
  "application",
  "client",
  "engine",
  "host",
  "port",
  "protocol",
  "sdk",
  "service",
  "transport",
]);
const categoryPrefixes = new Map([
  ["adapter", "adapters/"],
  ["application", "applications/"],
  ["client", "client/"],
  ["engine", "engine/"],
  ["host", "host/"],
  ["port", "ports/"],
  ["protocol", "protocol/"],
  ["sdk", "sdk/"],
  ["service", "service/"],
  ["transport", "transport/"],
]);
const allowedMaturities = new Set(["qualified", "adopted", "reference"]);
const allowedEffects = new Set([
  "pure",
  "port-only",
  "effectful-adapter",
  "composition-root",
  "facade",
]);
const allowedAudiences = new Set([
  "application",
  "remote-client",
  "sdk",
  "service",
  "trusted-host",
]);
const requiredFields = [
  "path",
  "category",
  "maturity",
  "effect_class",
  "audiences",
  "publish",
];

function fail(message) {
  throw new Error(`package classification: ${message}`);
}

function canonicalPath(path) {
  return path === "" ? "." : path.split("\\").join("/");
}

const classification = JSON.parse(readFileSync(classificationPath, "utf8"));
if (classification.schema !== 1) {
  fail("schema must be 1");
}
if (
  classification.packages === null ||
  typeof classification.packages !== "object" ||
  Array.isArray(classification.packages)
) {
  fail("packages must be an object");
}

const metadata = JSON.parse(
  execFileSync(
    "cargo",
    ["metadata", "--offline", "--locked", "--no-deps", "--format-version", "1"],
    { cwd: root, encoding: "utf8" },
  ),
);
const actual = new Map(
  metadata.packages.map((pkg) => [
    pkg.name,
    {
      path: canonicalPath(relative(root, dirname(pkg.manifest_path))),
      publish: Array.isArray(pkg.publish) && pkg.publish.length === 0,
    },
  ]),
);
const classified = new Set(Object.keys(classification.packages));
const missing = [...actual.keys()].filter((name) => !classified.has(name));
const stale = [...classified].filter((name) => !actual.has(name));
if (missing.length || stale.length) {
  fail(
    `inventory mismatch; missing=${missing.sort().join(",") || "none"}; stale=${stale.sort().join(",") || "none"}`,
  );
}

const seenPaths = new Map();
for (const name of [...classified].sort()) {
  const entry = classification.packages[name];
  if (entry === null || typeof entry !== "object" || Array.isArray(entry)) {
    fail(`${name} entry must be an object`);
  }
  const fields = Object.keys(entry).sort();
  const expectedFields = [...requiredFields].sort();
  if (JSON.stringify(fields) !== JSON.stringify(expectedFields)) {
    fail(`${name} fields must be exactly ${expectedFields.join(",")}`);
  }
  if (typeof entry.path !== "string" || canonicalPath(entry.path) !== entry.path) {
    fail(`${name} path must be a canonical relative string`);
  }
  if (entry.path.startsWith("/") || entry.path.includes("..")) {
    fail(`${name} path must stay beneath the repository root`);
  }
  if (entry.path !== actual.get(name).path) {
    fail(`${name} path ${entry.path} does not match ${actual.get(name).path}`);
  }
  if (seenPaths.has(entry.path)) {
    fail(`${name} duplicates path owned by ${seenPaths.get(entry.path)}`);
  }
  seenPaths.set(entry.path, name);
  if (!allowedCategories.has(entry.category)) {
    fail(`${name} has unknown category ${entry.category}`);
  }
  if (!entry.path.startsWith(categoryPrefixes.get(entry.category))) {
    fail(`${name} path does not match declared category ${entry.category}`);
  }
  if (!allowedMaturities.has(entry.maturity)) {
    fail(`${name} has unknown maturity ${entry.maturity}`);
  }
  if (!allowedEffects.has(entry.effect_class)) {
    fail(`${name} has unknown effect class ${entry.effect_class}`);
  }
  if (!Array.isArray(entry.audiences) || entry.audiences.length === 0) {
    fail(`${name} audiences must be a nonempty array`);
  }
  const sortedAudiences = [...entry.audiences].sort();
  if (JSON.stringify(sortedAudiences) !== JSON.stringify(entry.audiences)) {
    fail(`${name} audiences must be sorted and unique`);
  }
  for (const audience of entry.audiences) {
    if (!allowedAudiences.has(audience)) {
      fail(`${name} has unknown audience ${audience}`);
    }
  }
  if (entry.publish !== false || actual.get(name).publish !== true) {
    fail(`${name} must remain unpublished in classification and Cargo metadata`);
  }
}

const counts = Object.values(classification.packages).reduce((out, entry) => {
  out[entry.maturity] = (out[entry.maturity] ?? 0) + 1;
  return out;
}, {});
console.log(
  `Agora package classification closes over ${actual.size} packages (${counts.qualified ?? 0} qualified, ${counts.adopted ?? 0} adopted, ${counts.reference ?? 0} reference)`,
);
