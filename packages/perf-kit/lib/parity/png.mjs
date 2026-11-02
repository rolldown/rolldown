// Minimal PNG codec for screenshot comparison. The lab has no dependencies, and the
// only PNG files it ever reads are its own CDP screenshots (8-bit, non-interlaced),
// so a full decoder is ~100 lines of zlib + scanline unfiltering. Everything decodes
// to RGBA so the comparison loop never branches on color type.

import zlib from 'node:zlib';

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const CHANNELS = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 };

/** @returns {{ width: number, height: number, data: Uint8Array }} RGBA, 4 bytes per pixel */
export function decodePng(buf) {
  if (buf.length < 8 || !buf.subarray(0, 8).equals(SIGNATURE)) throw new Error('not a PNG');
  let offset = 8;
  let header = null;
  let palette = null;
  let transparency = null;
  const idat = [];
  while (offset + 8 <= buf.length) {
    const length = buf.readUInt32BE(offset);
    const type = buf.toString('latin1', offset + 4, offset + 8);
    const data = buf.subarray(offset + 8, offset + 8 + length);
    offset += 12 + length;
    if (type === 'IHDR') {
      header = {
        width: data.readUInt32BE(0),
        height: data.readUInt32BE(4),
        bitDepth: data[8],
        colorType: data[9],
        interlace: data[12],
      };
    } else if (type === 'PLTE') palette = data;
    else if (type === 'tRNS') transparency = data;
    else if (type === 'IDAT') idat.push(data);
    else if (type === 'IEND') break;
  }
  if (!header) throw new Error('PNG has no IHDR chunk');
  const { width, height, bitDepth, colorType, interlace } = header;
  const channels = CHANNELS[colorType];
  if (bitDepth !== 8 || interlace !== 0 || !channels) {
    throw new Error(
      `unsupported PNG (bit depth ${bitDepth}, color type ${colorType}, interlace ${interlace}) - only 8-bit non-interlaced screenshots are read`,
    );
  }
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  if (raw.length < (stride + 1) * height) throw new Error('PNG image data is truncated');
  const out = new Uint8Array(width * height * 4);
  let prev = new Uint8Array(stride);
  let cur = new Uint8Array(stride);
  for (let y = 0; y < height; y++) {
    const rowStart = y * (stride + 1);
    unfilter(raw[rowStart], raw.subarray(rowStart + 1, rowStart + 1 + stride), prev, cur, channels);
    expandRow(cur, out, y * width * 4, width, colorType, palette, transparency);
    [prev, cur] = [cur, prev];
  }
  return { width, height, data: out };
}

function paeth(a, b, c) {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  return pb <= pc ? b : c;
}

function unfilter(filter, line, prev, cur, bpp) {
  const n = line.length;
  switch (filter) {
    case 0:
      cur.set(line);
      return;
    case 1:
      for (let i = 0; i < n; i++) cur[i] = line[i] + (i >= bpp ? cur[i - bpp] : 0);
      return;
    case 2:
      for (let i = 0; i < n; i++) cur[i] = line[i] + prev[i];
      return;
    case 3:
      for (let i = 0; i < n; i++)
        cur[i] = line[i] + (((i >= bpp ? cur[i - bpp] : 0) + prev[i]) >> 1);
      return;
    case 4:
      for (let i = 0; i < n; i++) {
        cur[i] =
          line[i] + paeth(i >= bpp ? cur[i - bpp] : 0, prev[i], i >= bpp ? prev[i - bpp] : 0);
      }
      return;
    default:
      throw new Error(`PNG scanline has unknown filter type ${filter}`);
  }
}

function expandRow(cur, out, base, width, colorType, palette, transparency) {
  if (colorType === 6) {
    out.set(cur, base);
    return;
  }
  for (let x = 0, o = base; x < width; x++, o += 4) {
    if (colorType === 2) {
      out[o] = cur[x * 3];
      out[o + 1] = cur[x * 3 + 1];
      out[o + 2] = cur[x * 3 + 2];
      out[o + 3] = 255;
    } else if (colorType === 3) {
      const i = cur[x];
      out[o] = palette?.[i * 3] ?? 0;
      out[o + 1] = palette?.[i * 3 + 1] ?? 0;
      out[o + 2] = palette?.[i * 3 + 2] ?? 0;
      out[o + 3] = transparency?.[i] ?? 255;
    } else {
      const gray = colorType === 4 ? cur[x * 2] : cur[x];
      out[o] = gray;
      out[o + 1] = gray;
      out[o + 2] = gray;
      out[o + 3] = colorType === 4 ? cur[x * 2 + 1] : 255;
    }
  }
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(bytes) {
  let c = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) c = CRC_TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, 'latin1');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])), 0);
  return Buffer.concat([head, data, crc]);
}

/** RGBA -> PNG (filter 0 on every row; these are diagnostic images, not assets). */
export function encodePng({ width, height, data }) {
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    raw.set(data.subarray(y * stride, (y + 1) * stride), y * (stride + 1) + 1);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  return Buffer.concat([
    SIGNATURE,
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw, { level: 4 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}
