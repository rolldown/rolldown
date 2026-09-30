// Minimal, dependency-free reader/writer for the two wasm module sections the
// threaded WASI build tooling needs: the export section (names and indices) and
// the memory import (its declared minimum).
//
// Used by scripts/wasi/rename-wasm-allocator-exports.mjs,
// scripts/wasi/check-wasi-dist-files.mjs and
// packages/rolldown/tests/wasi/threaded-memory-stress.mjs.
// See internal-docs/wasi-shared-memory-grow/implementation.md

const MAGIC = [0x00, 0x61, 0x73, 0x6d];
const IMPORT_SECTION = 2;
const EXPORT_SECTION = 7;

export const EXPORT_KIND_FUNCTION = 0;

function readU32(bytes, offset) {
  let result = 0;
  let scale = 1;
  for (let i = 0; i < 5; i++) {
    const byte = bytes[offset + i];
    if (byte === undefined) throw new Error(`truncated LEB128 at byte ${offset}`);
    result += (byte & 0x7f) * scale;
    if ((byte & 0x80) === 0) return [result, offset + i + 1];
    scale *= 128;
  }
  throw new Error(`LEB128 longer than 5 bytes at byte ${offset}`);
}

function encodeU32(value) {
  const out = [];
  do {
    let byte = value % 128;
    value = Math.floor(value / 128);
    if (value !== 0) byte |= 0x80;
    out.push(byte);
  } while (value !== 0);
  return Uint8Array.from(out);
}

function readName(bytes, offset) {
  const [length, start] = readU32(bytes, offset);
  const end = start + length;
  return [new TextDecoder('utf-8', { fatal: true }).decode(bytes.subarray(start, end)), end];
}

/** Split a module into its sections: `{ id, start, end, contentStart }` (byte offsets). */
export function readSections(bytes) {
  if (bytes.length < 8 || MAGIC.some((byte, i) => bytes[i] !== byte)) {
    throw new Error('not a wasm module');
  }
  const sections = [];
  let offset = 8;
  while (offset < bytes.length) {
    const id = bytes[offset];
    const [size, contentStart] = readU32(bytes, offset + 1);
    const end = contentStart + size;
    if (end > bytes.length) throw new Error(`section ${id} runs past the end of the module`);
    sections.push({ id, start: offset, end, contentStart });
    offset = end;
  }
  return sections;
}

/** The export section as `[{ name, kind, index }]` in module order. */
export function readExports(bytes) {
  const section = readSections(bytes).find(({ id }) => id === EXPORT_SECTION);
  if (!section) return [];
  let [count, offset] = readU32(bytes, section.contentStart);
  const exports = [];
  while (count-- > 0) {
    let name;
    [name, offset] = readName(bytes, offset);
    const kind = bytes[offset];
    let index;
    [index, offset] = readU32(bytes, offset + 1);
    exports.push({ name, kind, index });
  }
  if (offset !== section.end) throw new Error('export section has trailing bytes');
  return exports;
}

/** A copy of `bytes` whose export section holds exactly `exports`; every other byte is kept. */
export function writeExports(bytes, exports) {
  const section = readSections(bytes).find(({ id }) => id === EXPORT_SECTION);
  if (!section) throw new Error('module has no export section');
  const encoder = new TextEncoder();
  const parts = [encodeU32(exports.length)];
  for (const { name, kind, index } of exports) {
    const encodedName = encoder.encode(name);
    parts.push(encodeU32(encodedName.length), encodedName, Uint8Array.of(kind), encodeU32(index));
  }
  const content = concat(parts);
  return concat([
    bytes.subarray(0, section.start),
    Uint8Array.of(EXPORT_SECTION),
    encodeU32(content.length),
    content,
    bytes.subarray(section.end),
  ]);
}

/** The imported memory's `{ module, name, minimum, maximum, shared }`, or null. */
export function readMemoryImport(bytes) {
  const section = readSections(bytes).find(({ id }) => id === IMPORT_SECTION);
  if (!section) return null;
  let [count, offset] = readU32(bytes, section.contentStart);
  while (count-- > 0) {
    let module, name;
    [module, offset] = readName(bytes, offset);
    [name, offset] = readName(bytes, offset);
    const kind = bytes[offset++];
    switch (kind) {
      case 0: // function: type index
        [, offset] = readU32(bytes, offset);
        break;
      case 1: // table: reference type, limits
        offset = readLimits(bytes, offset + 1).offset;
        break;
      case 2: {
        const { minimum, maximum, shared } = readLimits(bytes, offset);
        return { module, name, minimum, maximum, shared };
      }
      case 3: // global: value type, mutability
        offset += 2;
        break;
      case 4: // tag: attribute, type index
        [, offset] = readU32(bytes, offset + 1);
        break;
      default:
        throw new Error(`unknown import kind ${kind} for ${module}.${name}`);
    }
  }
  return null;
}

function readLimits(bytes, offset) {
  const flags = bytes[offset];
  if (flags > 3) throw new Error(`unsupported limits flags 0x${flags.toString(16)}`);
  let minimum, maximum;
  [minimum, offset] = readU32(bytes, offset + 1);
  if (flags & 1) [maximum, offset] = readU32(bytes, offset);
  return { minimum, maximum, shared: (flags & 2) !== 0, offset };
}

function concat(parts) {
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}
