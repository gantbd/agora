import { lstatSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const BLOCK = 512;
const MAX_ARCHIVE_BYTES = 64 * 1024 * 1024;
const MAX_FILE_BYTES = 16 * 1024 * 1024;
const MAX_ENTRIES = 10_000;
const ROOT = 'agora-source/';

function fail(message) {
  throw new Error(`canonical USTAR: ${message}`);
}

function assertPath(path, directory) {
  if (!path.startsWith(ROOT) || path.includes('..') || path.startsWith('/') ||
      !/^[A-Za-z0-9._/-]+$/.test(path) || path.includes('//') ||
      (directory !== path.endsWith('/'))) {
    fail(`unsafe or noncanonical path: ${path}`);
  }
}

function splitPath(path) {
  const bytes = Buffer.byteLength(path);
  if (bytes <= 100) return { name: path, prefix: '' };
  const directory = path.endsWith('/');
  const body = directory ? path.slice(0, -1) : path;
  for (let index = body.lastIndexOf('/'); index > 0; index = body.lastIndexOf('/', index - 1)) {
    const prefix = body.slice(0, index);
    const name = `${body.slice(index + 1)}${directory ? '/' : ''}`;
    if (Buffer.byteLength(prefix) <= 155 && Buffer.byteLength(name) <= 100) {
      return { name, prefix };
    }
  }
  fail(`path does not fit USTAR fields: ${path}`);
}

function writeText(header, offset, length, value) {
  const encoded = Buffer.from(value, 'ascii');
  if (encoded.length > length) fail(`field is too long: ${value}`);
  encoded.copy(header, offset);
}

function writeOctal(header, offset, length, value) {
  if (!Number.isSafeInteger(value) || value < 0) fail('invalid numeric field');
  const encoded = value.toString(8).padStart(length - 1, '0');
  if (encoded.length !== length - 1) fail('numeric field overflow');
  writeText(header, offset, length - 1, encoded);
  header[offset + length - 1] = 0;
}

function writeChecksum(header) {
  header.fill(0x20, 148, 156);
  const sum = header.reduce((total, byte) => total + byte, 0);
  const encoded = sum.toString(8).padStart(6, '0');
  if (encoded.length !== 6) fail('checksum overflow');
  writeText(header, 148, 6, encoded);
  header[154] = 0;
  header[155] = 0x20;
}

function headerFor(entry) {
  const header = Buffer.alloc(BLOCK);
  const { name, prefix } = splitPath(entry.path);
  writeText(header, 0, 100, name);
  writeOctal(header, 100, 8, entry.mode);
  writeOctal(header, 108, 8, 0);
  writeOctal(header, 116, 8, 0);
  writeOctal(header, 124, 12, entry.data.length);
  writeOctal(header, 136, 12, 0);
  header[156] = entry.directory ? 0x35 : 0x30;
  writeText(header, 257, 6, 'ustar\0');
  writeText(header, 263, 2, '00');
  writeText(header, 265, 32, 'root');
  writeText(header, 297, 32, 'root');
  writeText(header, 345, 155, prefix);
  writeChecksum(header);
  return header;
}

export function encodeCanonicalUstar(entries) {
  if (!Array.isArray(entries) || entries.length === 0 || entries.length > MAX_ENTRIES) {
    fail('entry inventory is empty or over limit');
  }
  let previous = '';
  const directories = new Set();
  const chunks = [];
  let total = 2 * BLOCK;
  for (const [index, entry] of entries.entries()) {
    if (!entry || !Buffer.isBuffer(entry.data)) fail('entry data is not a buffer');
    assertPath(entry.path, entry.directory);
    if (index === 0 && (entry.path !== ROOT || !entry.directory)) {
      fail('archive does not begin with its exact root directory');
    }
    if (entry.path <= previous) fail('entries are duplicated or not strictly ordered');
    previous = entry.path;
    if (entry.path !== ROOT) {
      const body = entry.directory ? entry.path.slice(0, -1) : entry.path;
      const parent = `${body.slice(0, body.lastIndexOf('/') + 1)}`;
      if (!directories.has(parent)) fail(`entry parent is absent: ${entry.path}`);
    }
    if (entry.directory) directories.add(entry.path);
    if (entry.data.length > MAX_FILE_BYTES || (entry.directory && entry.data.length !== 0)) {
      fail('entry size is invalid');
    }
    const expectedMode = entry.directory ? 0o755 : (entry.executable ? 0o755 : 0o644);
    if (entry.mode !== expectedMode) fail('entry mode is noncanonical');
    chunks.push(headerFor(entry), entry.data);
    const padding = (BLOCK - (entry.data.length % BLOCK)) % BLOCK;
    if (padding) chunks.push(Buffer.alloc(padding));
    total += BLOCK + entry.data.length + padding;
    if (total > MAX_ARCHIVE_BYTES) fail('archive exceeds the byte ceiling');
  }
  chunks.push(Buffer.alloc(2 * BLOCK));
  return Buffer.concat(chunks, total);
}

function collect(root, relative = '') {
  const entries = [];
  const names = readdirSync(join(root, relative), { withFileTypes: true })
    .map((entry) => entry.name).sort();
  for (const name of names) {
    const child = relative ? `${relative}/${name}` : name;
    const absolute = join(root, child);
    const stat = lstatSync(absolute);
    if (stat.isSymbolicLink() || (!stat.isDirectory() && !stat.isFile())) {
      fail(`symbolic or special input: ${child}`);
    }
    if (stat.isDirectory()) {
      entries.push({ path: `${ROOT}${child}/`, directory: true, executable: true,
        mode: 0o755, data: Buffer.alloc(0) });
      entries.push(...collect(root, child));
    } else {
      const data = readFileSync(absolute);
      entries.push({ path: `${ROOT}${child}`, directory: false,
        executable: (stat.mode & 0o111) !== 0, mode: (stat.mode & 0o111) !== 0 ? 0o755 : 0o644,
        data });
    }
  }
  return entries;
}

export function buildCanonicalUstar(root) {
  const stat = lstatSync(root);
  if (!stat.isDirectory() || stat.isSymbolicLink()) fail('input root is not a real directory');
  const entries = [{ path: ROOT, directory: true, executable: true,
    mode: 0o755, data: Buffer.alloc(0) }, ...collect(root)];
  entries.sort((left, right) => left.path < right.path ? -1 : left.path > right.path ? 1 : 0);
  return encodeCanonicalUstar(entries);
}

function readText(block, offset, length) {
  const field = block.subarray(offset, offset + length);
  const end = field.indexOf(0);
  return field.subarray(0, end === -1 ? field.length : end).toString('ascii');
}

function readOctal(block, offset, length) {
  const text = readText(block, offset, length).trim();
  if (!/^[0-7]+$/.test(text)) fail('malformed numeric field');
  const value = Number.parseInt(text, 8);
  if (!Number.isSafeInteger(value)) fail('numeric field overflow');
  return value;
}

function zeroBlock(block) {
  return block.every((byte) => byte === 0);
}

export function validateCanonicalUstar(archive) {
  if (!Buffer.isBuffer(archive) || archive.length < 3 * BLOCK ||
      archive.length > MAX_ARCHIVE_BYTES || archive.length % BLOCK !== 0) {
    fail('archive size or alignment is invalid');
  }
  const entries = [];
  let offset = 0;
  while (offset + 2 * BLOCK <= archive.length) {
    const header = archive.subarray(offset, offset + BLOCK);
    if (zeroBlock(header)) break;
    const checksum = readOctal(header, 148, 8);
    const copy = Buffer.from(header);
    copy.fill(0x20, 148, 156);
    if (copy.reduce((total, byte) => total + byte, 0) !== checksum) fail('header checksum mismatch');
    const name = readText(header, 0, 100);
    const prefix = readText(header, 345, 155);
    const path = prefix ? `${prefix}/${name}` : name;
    const type = String.fromCharCode(header[156]);
    const directory = type === '5';
    if (!directory && type !== '0') fail('entry type is not a regular file or directory');
    const size = readOctal(header, 124, 12);
    const mode = readOctal(header, 100, 8);
    const dataStart = offset + BLOCK;
    const dataEnd = dataStart + size;
    const next = dataStart + Math.ceil(size / BLOCK) * BLOCK;
    if (dataEnd > archive.length || next > archive.length) fail('entry extends beyond archive');
    const data = Buffer.from(archive.subarray(dataStart, dataEnd));
    entries.push({ path, directory, executable: mode === 0o755, mode, data });
    offset = next;
    if (entries.length > MAX_ENTRIES) fail('entry ceiling exceeded');
  }
  if (offset + 2 * BLOCK !== archive.length ||
      !zeroBlock(archive.subarray(offset, offset + BLOCK)) ||
      !zeroBlock(archive.subarray(offset + BLOCK, offset + 2 * BLOCK))) {
    fail('archive does not end with exactly two zero blocks');
  }
  const canonical = encodeCanonicalUstar(entries);
  if (!canonical.equals(archive)) fail('archive bytes are not canonical');
  return entries.map(({ path, directory, mode, data }) => ({ path, directory, mode, size: data.length }));
}
