import { readFileSync } from 'node:fs';
import { validateSourceSbom } from './source-sbom-lib.mjs';

const original = JSON.parse(readFileSync(new URL('../docs/agora-source.cdx.json', import.meta.url)));
const clone = () => structuredClone(original);
const mutants = [];
let mutant = clone();
mutant.components.pop();
mutants.push(['missing component', mutant]);
mutant = clone();
mutant.components[0].licenses[0].expression = 'UNKNOWN';
mutants.push(['changed license', mutant]);
mutant = clone();
mutant.dependencies[0].dependsOn.push('pkg:cargo/unknown@0.0.0');
mutants.push(['escaped dependency', mutant]);
mutant = clone();
mutant.components[0].properties.push({ name: 'path', value: '/Users/example/source' });
mutants.push(['host path', mutant]);
mutant = clone();
mutant.components[0].properties.find((entry) => entry.name === 'agora:publication').value = 'allowed';
mutants.push(['publication enabled', mutant]);

for (const [label, candidate] of mutants) {
  let refused = false;
  try {
    validateSourceSbom(candidate);
  } catch {
    refused = true;
  }
  if (!refused) throw new Error(`source SBOM validator accepted mutant: ${label}`);
}
console.log('Agora source SBOM rejects inventory, license, edge, path, and publication mutants');
