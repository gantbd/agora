import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { buildSourceSbom, validateSourceSbom } from './source-sbom-lib.mjs';

if (process.argv.length !== 3) {
  throw new Error('usage: build-source-sbom.mjs OUTPUT_FILE');
}
const metadata = JSON.parse(execFileSync('cargo', [
  'metadata', '--format-version', '1', '--locked', '--offline',
], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }));
const bom = buildSourceSbom(metadata);
validateSourceSbom(bom);
writeFileSync(process.argv[2], `${JSON.stringify(bom, null, 2)}\n`, { flag: 'wx' });
