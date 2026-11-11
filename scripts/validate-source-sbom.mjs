import { readFileSync } from 'node:fs';
import { validateSourceSbom } from './source-sbom-lib.mjs';

if (process.argv.length !== 3) throw new Error('usage: validate-source-sbom.mjs SBOM_FILE');
validateSourceSbom(JSON.parse(readFileSync(process.argv[2], 'utf8')));
