import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmodSync, cpSync, mkdirSync, mkdtempSync, readFileSync, readdirSync,
  rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';

const scripts = dirname(fileURLToPath(import.meta.url));
const work = mkdtempSync(join(tmpdir(), 'agora-source-archive-contract.'));
process.on('exit', () => rmSync(work, { recursive: true, force: true }));
const candidate = join(work, 'candidate');

function put(path, content = '') {
  const target = join(candidate, path);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, content);
}

put('SOURCE-MANIFEST.txt', `schema=agora-source-candidate-v1
agora_revision=${'1'.repeat(40)}
aegis_context_revision=${'2'.repeat(40)}
aegis_selected_tree_clean=false
keystone_context_revision=${'3'.repeat(40)}
keystone_selected_tree_clean=false
stable_component_count=19
provenance=exact-local-content-closure
claim=self-contained-stable-local-source
publication=denied
`);
for (const path of ['agora/Cargo.toml', 'agora/Cargo.lock',
  'aegis/crates/aegis-client/Cargo.toml', 'aegis/crates/aegis-protocol/Cargo.toml',
  'keystone/Cargo.toml', 'keystone/crates/keystone-codec/Cargo.toml',
  'keystone/crates/keystone-storage/Cargo.toml']) put(path, `[package]\nname = "fixture"\n`);
put('agora/scripts/run.sh', '#!/bin/sh\nexit 0\n');
chmodSync(join(candidate, 'agora/scripts/run.sh'), 0o755);

function files(root, relative = '') {
  return readdirSync(join(root, relative), { withFileTypes: true }).flatMap((entry) => {
    const child = relative ? `${relative}/${entry.name}` : entry.name;
    return entry.isDirectory() ? files(root, child) : [child];
  });
}
const inventory = files(candidate).sort().map((path) => {
  const digest = createHash('sha256').update(readFileSync(join(candidate, path))).digest('hex');
  return `${digest}  ./${path}`;
});
writeFileSync(join(candidate, 'SHA256SUMS'), `${inventory.join('\n')}\n`);

const one = join(work, 'one.tar');
const two = join(work, 'two.tar');
const build = resolve(scripts, 'build-source-archive.sh');
const verify = resolve(scripts, 'verify-source-archive.sh');
execFileSync(build, [candidate, one], { stdio: 'pipe' });
execFileSync(build, [candidate, two], { stdio: 'pipe' });
if (!readFileSync(one).equals(readFileSync(two))) throw new Error('independent archives differ');
execFileSync(verify, [one], { stdio: 'pipe' });

const listing = execFileSync('tar', ['-tf', one], { encoding: 'utf8' }).trim().split('\n');
if (listing[0] !== 'agora-source/' || !listing.includes('agora-source/SHA256SUMS') ||
    !listing.includes('agora-source/agora/scripts/run.sh')) throw new Error('system tar listing disagrees');
const extracted = join(work, 'extracted');
mkdirSync(extracted);
execFileSync('tar', ['-xf', one, '-C', extracted]);
if (readFileSync(join(extracted, 'agora-source/agora/scripts/run.sh'), 'utf8') !== '#!/bin/sh\nexit 0\n') {
  throw new Error('system tar extraction changed content');
}

function rewriteChecksum(path) {
  const digest = createHash('sha256').update(readFileSync(path)).digest('hex');
  writeFileSync(`${path}.sha256`, `${digest}  ${basename(path)}\n`);
}
function expectRefusal(label, path) {
  let refused = false;
  try { execFileSync(verify, [path], { stdio: 'pipe' }); } catch { refused = true; }
  if (!refused) throw new Error(`source archive mutant passed: ${label}`);
}

let mutant = Buffer.from(readFileSync(one));
mutant[mutant.length - 1025] ^= 1;
let path = join(work, 'changed.tar');
writeFileSync(path, mutant);
cpSync(`${one}.sha256`, `${path}.sha256`);
expectRefusal('changed bytes', path);

path = join(work, 'truncated.tar');
writeFileSync(path, readFileSync(one).subarray(0, -512));
rewriteChecksum(path);
expectRefusal('truncation with matching checksum', path);

path = join(work, 'trailing.tar');
writeFileSync(path, Buffer.concat([readFileSync(one), Buffer.alloc(512)]));
rewriteChecksum(path);
expectRefusal('trailing zero block', path);

path = join(work, 'rootless.tar');
writeFileSync(path, readFileSync(one).subarray(512));
rewriteChecksum(path);
expectRefusal('missing canonical root entry', path);

mutant = Buffer.from(readFileSync(one));
mutant.fill(0, 512, 612);
Buffer.from('../escape', 'ascii').copy(mutant, 512);
mutant.fill(0x20, 512 + 148, 512 + 156);
const sum = mutant.subarray(512, 1024).reduce((total, byte) => total + byte, 0);
Buffer.from(sum.toString(8).padStart(6, '0'), 'ascii').copy(mutant, 512 + 148);
mutant[512 + 154] = 0;
mutant[512 + 155] = 0x20;
path = join(work, 'escape.tar');
writeFileSync(path, mutant);
rewriteChecksum(path);
expectRefusal('path traversal with valid header checksum', path);

symlinkSync('Cargo.toml', join(candidate, 'agora/link'));
let refused = false;
try { execFileSync(build, [candidate, join(work, 'link.tar')], { stdio: 'pipe' }); } catch { refused = true; }
if (!refused) throw new Error('symbolic candidate input passed');

console.log('Agora source-archive contract is deterministic, interoperable, and rejects checksum, framing, path, and link mutants');
