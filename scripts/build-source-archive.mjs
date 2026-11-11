import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildCanonicalUstar, validateCanonicalUstar } from './canonical-ustar.mjs';

if (process.argv.length !== 4) throw new Error('usage: build-source-archive.mjs CANDIDATE OUTPUT');
const scriptRoot = dirname(fileURLToPath(import.meta.url));
const candidate = resolve(process.argv[2]);
const output = resolve(process.argv[3]);
execFileSync(resolve(scriptRoot, 'verify-source-candidate.sh'), [candidate], { stdio: 'inherit' });
const archive = buildCanonicalUstar(candidate);
validateCanonicalUstar(archive);
writeFileSync(output, archive, { flag: 'wx', mode: 0o644 });
console.log(`Agora canonical source archive built at ${output}`);
