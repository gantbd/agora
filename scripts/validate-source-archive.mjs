import { readFileSync } from 'node:fs';
import { validateCanonicalUstar } from './canonical-ustar.mjs';

if (process.argv.length !== 3) throw new Error('usage: validate-source-archive.mjs ARCHIVE');
const entries = validateCanonicalUstar(readFileSync(process.argv[2]));
console.log(`Agora canonical source archive verified (${entries.length} entries)`);
