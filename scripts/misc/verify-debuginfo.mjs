// Check that a stripped release binding carries no debug info, and that its
// published debug info archive holds the file a backtrace would load for it.
// The check reads the ID each format uses to match the two files, without running the binding.
// See internal-docs/panic-symbolication/implementation.md
//
// Usage: node scripts/misc/verify-debuginfo.mjs [--binding <file.node>] [--debuginfo <archive.tar.zst>]
// Without `--binding`, the single `.node` under `packages/rolldown/src/` is used.
// Without `--debuginfo`, the single archive under `target/debuginfo/` is used.

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { crc32 } from 'node:zlib';

const REPO_ROOT = fileURLToPath(new URL('../../', import.meta.url));

function findOne(dir, re) {
  const files = fs.readdirSync(dir).filter((f) => re.test(f));
  if (files.length !== 1)
    throw new Error(`expected one ${re} in ${dir}, found: ${files.join(', ') || 'none'}`);
  return path.join(dir, files[0]);
}

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--binding') args.binding = path.resolve(argv[++i]);
    else if (argv[i] === '--debuginfo') args.debuginfo = path.resolve(argv[++i]);
    else throw new Error(`unknown argument: ${argv[i]}`);
  }
  args.binding ??= findOne(
    path.join(REPO_ROOT, 'packages/rolldown/src'),
    /^rolldown-binding\..+\.node$/,
  );
  args.debuginfo ??= findOne(path.join(REPO_ROOT, 'target/debuginfo'), /\.debuginfo\.tar\.zst$/);
  return args;
}

function cstr(buf, start, maxLen = buf.length - start) {
  const end = buf.indexOf(0, start);
  return buf.toString('latin1', start, end === -1 || end > start + maxLen ? start + maxLen : end);
}

function assert(cond, message) {
  if (!cond) throw new Error(message);
}

// ELF: backtrace-rs follows the binding's `.gnu_debuglink` record, which names the
// debug file and holds its CRC32.
const SHT_NOBITS = 8;

function elfSections(buf) {
  assert(buf[4] === 2 && buf[5] === 1, 'only 64-bit little-endian ELF is supported');
  const shoff = Number(buf.readBigUInt64LE(0x28));
  const shentsize = buf.readUInt16LE(0x3a);
  const shnum = buf.readUInt16LE(0x3c);
  const headers = Array.from({ length: shnum }, (_, i) => {
    const o = shoff + i * shentsize;
    const offset = Number(buf.readBigUInt64LE(o + 0x18));
    const size = Number(buf.readBigUInt64LE(o + 0x20));
    return { nameOff: buf.readUInt32LE(o), type: buf.readUInt32LE(o + 4), offset, size };
  });
  const names = headers[buf.readUInt16LE(0x3e)];
  return new Map(
    headers.map((h) => [
      cstr(buf, names.offset + h.nameOff),
      h.type === SHT_NOBITS ? Buffer.alloc(0) : buf.subarray(h.offset, h.offset + h.size),
    ]),
  );
}

function verifyElf(binding, bindingName, entry, dir) {
  const sections = elfSections(binding);
  const leftover = [...sections.keys()].filter((n) => n.startsWith('.debug_'));
  assert(leftover.length === 0, `debug sections remain in the binding: ${leftover.join(', ')}`);

  const link = sections.get('.gnu_debuglink');
  assert(link, 'the binding has no .gnu_debuglink section');
  const linkName = cstr(link, 0);
  // The CRC follows the name, its NUL, and padding to 4 bytes.
  const linkCrc = link.readUInt32LE((linkName.length + 4) & ~3);
  assert(linkName === `${bindingName}.debug`, `.gnu_debuglink names ${linkName}`);
  assert(entry === linkName, `the archive holds ${entry}, but .gnu_debuglink names ${linkName}`);

  const debug = fs.readFileSync(path.join(dir, entry));
  assert(crc32(debug) === linkCrc, `CRC32 of ${entry} does not match .gnu_debuglink`);
  const debugSections = elfSections(debug);
  for (const name of ['.debug_info', '.debug_line']) {
    assert(debugSections.get(name)?.length, `${entry} has no ${name}`);
  }
  // Only a linker that writes a build ID gives one to check.
  const buildId = sections.get('.note.gnu.build-id');
  if (buildId) {
    assert(
      buildId.equals(debugSections.get('.note.gnu.build-id') ?? Buffer.alloc(0)),
      `the build ID of ${entry} does not match the binding`,
    );
  }
  return `.gnu_debuglink ${linkName}, CRC32 ${linkCrc.toString(16)}`;
}

// Mach-O: backtrace-rs scans the binding's directory for `*.dSYM` and matches by LC_UUID.
const LC_SYMTAB = 0x2;
const LC_SEGMENT_64 = 0x19;
const LC_UUID = 0x1b;
const N_STAB = 0xe0;
// `strip` always keeps one `radr://5614542` N_OPT marker, which is not debug info.
const N_OPT = 0x3c;

function machO(buf) {
  assert(buf.readUInt32LE(0) === 0xfeedfacf, 'only thin 64-bit Mach-O is supported');
  const info = { uuid: undefined, sections: new Map(), stabs: 0 };
  let off = 32;
  for (let i = 0, ncmds = buf.readUInt32LE(16); i < ncmds; i++) {
    const cmd = buf.readUInt32LE(off);
    if (cmd === LC_UUID) {
      info.uuid = buf.toString('hex', off + 8, off + 24);
    } else if (cmd === LC_SEGMENT_64) {
      for (let s = 0, nsects = buf.readUInt32LE(off + 64); s < nsects; s++) {
        const o = off + 72 + s * 80;
        const name = `${cstr(buf, o + 16, 16)},${cstr(buf, o, 16)}`;
        info.sections.set(name, Number(buf.readBigUInt64LE(o + 40)));
      }
    } else if (cmd === LC_SYMTAB) {
      const symoff = buf.readUInt32LE(off + 8);
      for (let s = 0, nsyms = buf.readUInt32LE(off + 12); s < nsyms; s++) {
        const type = buf[symoff + s * 16 + 4];
        if (type & N_STAB && type !== N_OPT) info.stabs++;
      }
    }
    off += buf.readUInt32LE(off + 4);
  }
  return info;
}

function verifyMachO(binding, _bindingName, entry, dir) {
  const node = machO(binding);
  assert(node.uuid, 'the binding has no LC_UUID');
  const dwarfSections = [...node.sections.keys()].filter((n) => n.startsWith('__DWARF,'));
  assert(
    dwarfSections.length === 0,
    `debug sections remain in the binding: ${dwarfSections.join(', ')}`,
  );
  assert(node.stabs === 0, `the binding still has ${node.stabs} debug map symbols`);

  assert(entry.endsWith('.dSYM'), `the archive holds ${entry}, not a .dSYM bundle`);
  const dwarf = findOne(path.join(dir, entry, 'Contents/Resources/DWARF'), /./);
  const dsym = machO(fs.readFileSync(dwarf));
  assert(dsym.uuid === node.uuid, `UUID ${dsym.uuid} of ${entry} does not match ${node.uuid}`);
  for (const name of ['__DWARF,__debug_info', '__DWARF,__debug_line']) {
    assert(dsym.sections.get(name), `${entry} has no ${name}`);
  }
  return `LC_UUID ${node.uuid}`;
}

// PE: dbghelp matches the GUID and age in the DLL's CodeView record against the PDB.
const IMAGE_DEBUG_TYPE_CODEVIEW = 2;

function peCodeView(buf) {
  const pe = buf.readUInt32LE(0x3c);
  assert(buf.readUInt32LE(pe) === 0x4550, 'no PE signature');
  const nsections = buf.readUInt16LE(pe + 6);
  const opt = pe + 24;
  assert(buf.readUInt16LE(opt) === 0x20b, 'only PE32+ is supported');
  const sectionTable = opt + buf.readUInt16LE(pe + 20);
  const toOffset = (rva) => {
    for (let i = 0; i < nsections; i++) {
      const s = sectionTable + i * 40;
      const va = buf.readUInt32LE(s + 12);
      if (rva >= va && rva < va + buf.readUInt32LE(s + 16))
        return buf.readUInt32LE(s + 20) + rva - va;
    }
    throw new Error(`RVA ${rva} is in no section`);
  };
  // Data directory 6 is the debug directory; each entry is 28 bytes.
  const dir = toOffset(buf.readUInt32LE(opt + 112 + 6 * 8));
  for (let i = 0, n = buf.readUInt32LE(opt + 112 + 6 * 8 + 4) / 28; i < n; i++) {
    const e = dir + i * 28;
    if (buf.readUInt32LE(e + 12) !== IMAGE_DEBUG_TYPE_CODEVIEW) continue;
    const p = buf.readUInt32LE(e + 24);
    assert(buf.toString('latin1', p, p + 4) === 'RSDS', 'the CodeView record is not RSDS');
    return {
      guid: buf.toString('hex', p + 4, p + 20),
      age: buf.readUInt32LE(p + 20),
      pdb: cstr(buf, p + 24),
    };
  }
  throw new Error('the binding has no CodeView record');
}

// Reads stream 1 of an MSF 7.0 file, the PDB info stream: version, signature, age, GUID.
function pdbInfo(buf) {
  assert(buf.toString('latin1', 0, 29) === 'Microsoft C/C++ MSF 7.00\r\n\x1aDS', 'not a PDB');
  const blockSize = buf.readUInt32LE(32);
  const block = (i) => buf.subarray(i * blockSize, (i + 1) * blockSize);
  const dirBytes = buf.readUInt32LE(44);
  const dirMap = block(buf.readUInt32LE(52));
  const dir = Buffer.concat(
    Array.from({ length: Math.ceil(dirBytes / blockSize) }, (_, i) =>
      block(dirMap.readUInt32LE(i * 4)),
    ),
  );
  const numStreams = dir.readUInt32LE(0);
  // A size of 0xffffffff marks an unused stream, which has no blocks.
  const blocks = (s) => {
    const size = dir.readUInt32LE(4 + s * 4);
    return size === 0xffffffff ? 0 : Math.ceil(size / blockSize);
  };
  // Block lists follow the sizes, stream by stream; stream 0's list comes before stream 1's.
  const stream1 = 4 + numStreams * 4 + blocks(0) * 4;
  const info = Buffer.concat(
    Array.from({ length: blocks(1) }, (_, i) => block(dir.readUInt32LE(stream1 + i * 4))),
  );
  return { age: info.readUInt32LE(8), guid: info.toString('hex', 12, 28) };
}

function verifyPe(binding, _bindingName, entry, dir) {
  const cv = peCodeView(binding);
  const pdbName = path.win32.basename(cv.pdb);
  assert(entry === pdbName, `the archive holds ${entry}, but the binding names ${pdbName}`);
  const pdb = pdbInfo(fs.readFileSync(path.join(dir, entry)));
  assert(
    pdb.guid === cv.guid && pdb.age === cv.age,
    `GUID ${pdb.guid} age ${pdb.age} of ${entry} does not match ${cv.guid} age ${cv.age}`,
  );
  return `CodeView GUID ${cv.guid} age ${cv.age}`;
}

function verifierFor(buf) {
  if (buf.readUInt32BE(0) === 0x7f454c46) return verifyElf;
  if (buf.readUInt32LE(0) === 0xfeedfacf) return verifyMachO;
  if (buf.readUInt16LE(0) === 0x5a4d) return verifyPe;
  throw new Error('unknown binary format');
}

function main() {
  const { binding, debuginfo } = parseArgs(process.argv.slice(2));
  const bindingName = path.basename(binding);
  assert(
    path.basename(debuginfo) === `${bindingName}.debuginfo.tar.zst`,
    `${path.basename(debuginfo)} is not named after ${bindingName}`,
  );
  const buf = fs.readFileSync(binding);
  const verify = verifierFor(buf);

  // bsdtar on Windows reads an absolute `C:\...` argument as `host:path`, so tar
  // runs inside the unpack directory and receives relative names.
  const dir = fs.mkdtempSync(path.join(path.dirname(debuginfo), '.verify-'));
  try {
    execFileSync('zstd', ['-d', '-q', '-f', debuginfo, '-o', path.join(dir, 'debuginfo.tar')]);
    const entries = execFileSync('tar', ['-tf', 'debuginfo.tar'], { cwd: dir, encoding: 'utf8' })
      .split('\n')
      .filter(Boolean)
      .map((l) => l.split('/')[0]);
    const tops = [...new Set(entries)];
    assert(tops.length === 1, `expected one entry in the archive, found: ${tops.join(', ')}`);
    execFileSync('tar', ['-xf', 'debuginfo.tar'], { cwd: dir });
    console.info(`${bindingName}: ${verify(buf, bindingName, tops[0], dir)}`);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
  console.info('ok');
}

main();
