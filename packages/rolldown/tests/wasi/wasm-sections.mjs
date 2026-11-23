// Minimal, dependency-free reader for the wasm module sections
// threaded-memory-stress.mjs needs: the import section (the memory import's
// declared minimum), the export section (names and indices) and the code
// section (one function's body, to see what a forwarder calls).
// See internal-docs/async-runtime/implementation.md, "Threaded WASI heap sync".

const MAGIC = [0x00, 0x61, 0x73, 0x6d];
const IMPORT_SECTION = 2;
const EXPORT_SECTION = 7;
const CODE_SECTION = 10;

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

function readName(bytes, offset) {
  const [length, start] = readU32(bytes, offset);
  const end = start + length;
  return [new TextDecoder('utf-8', { fatal: true }).decode(bytes.subarray(start, end)), end];
}

/** Split a module into its sections: `{ id, start, end, contentStart }` (byte offsets). */
function readSections(bytes) {
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

/**
 * The import section as `[{ module, name, kind }]` in module order; a memory
 * import also has `{ minimum, maximum, shared }`.
 */
function readImports(bytes) {
  const section = readSections(bytes).find(({ id }) => id === IMPORT_SECTION);
  if (!section) return [];
  let [count, offset] = readU32(bytes, section.contentStart);
  const imports = [];
  while (count-- > 0) {
    let module, name;
    [module, offset] = readName(bytes, offset);
    [name, offset] = readName(bytes, offset);
    const kind = bytes[offset++];
    switch (kind) {
      case 0: // function: type index
        [, offset] = readU32(bytes, offset);
        imports.push({ module, name, kind });
        break;
      case 1: // table: reference type, limits
        offset = readLimits(bytes, offset + 1).offset;
        imports.push({ module, name, kind });
        break;
      case 2: {
        const limits = readLimits(bytes, offset);
        offset = limits.offset;
        const { minimum, maximum, shared } = limits;
        imports.push({ module, name, kind, minimum, maximum, shared });
        break;
      }
      case 3: // global: value type, mutability
        offset += 2;
        imports.push({ module, name, kind });
        break;
      case 4: // tag: attribute, type index
        [, offset] = readU32(bytes, offset + 1);
        imports.push({ module, name, kind });
        break;
      default:
        throw new Error(`unknown import kind ${kind} for ${module}.${name}`);
    }
  }
  if (offset !== section.end) throw new Error('import section has trailing bytes');
  return imports;
}

/** The imported memory's `{ module, name, minimum, maximum, shared }`, or null. */
export function readMemoryImport(bytes) {
  const memory = readImports(bytes).find(({ kind }) => kind === 2);
  if (!memory) return null;
  const { module, name, minimum, maximum, shared } = memory;
  return { module, name, minimum, maximum, shared };
}

const OPCODE_END = 0x0b;
const OPCODE_CALL = 0x10;
const OPCODE_RETURN_CALL = 0x12;
const OPCODE_LOCAL_GET = 0x20;

/**
 * The function a one-argument forwarder at `functionIndex` calls: its body is
 * exactly `local.get 0; call X; end` (or `return_call X`) with no locals.
 * `undefined` for an imported function or any other body.
 */
export function readForwardTarget(bytes, functionIndex) {
  const importedFunctions = readImports(bytes).filter(({ kind }) => kind === 0).length;
  const bodyIndex = functionIndex - importedFunctions;
  if (bodyIndex < 0) return undefined;
  const section = readSections(bytes).find(({ id }) => id === CODE_SECTION);
  if (!section) return undefined;
  const [count, bodiesStart] = readU32(bytes, section.contentStart);
  if (bodyIndex >= count) return undefined;
  let offset = bodiesStart;
  for (let i = 0; i < bodyIndex; i++) {
    const [size, start] = readU32(bytes, offset);
    offset = start + size;
  }
  const [size, start] = readU32(bytes, offset);
  const end = start + size;
  const [localGroups, opcodeAt] = readU32(bytes, start);
  if (localGroups !== 0 || bytes[opcodeAt] !== OPCODE_LOCAL_GET) return undefined;
  const [local, callAt] = readU32(bytes, opcodeAt + 1);
  const opcode = bytes[callAt];
  if (local !== 0 || (opcode !== OPCODE_CALL && opcode !== OPCODE_RETURN_CALL)) return undefined;
  const [target, endAt] = readU32(bytes, callAt + 1);
  return bytes[endAt] === OPCODE_END && endAt + 1 === end ? target : undefined;
}

function readLimits(bytes, offset) {
  const flags = bytes[offset];
  if (flags > 3) throw new Error(`unsupported limits flags 0x${flags.toString(16)}`);
  let minimum, maximum;
  [minimum, offset] = readU32(bytes, offset + 1);
  if (flags & 1) [maximum, offset] = readU32(bytes, offset);
  return { minimum, maximum, shared: (flags & 2) !== 0, offset };
}
