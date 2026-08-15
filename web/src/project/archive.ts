// A minimal, deterministic ZIP writer.
//
// Zip rather than tar because a student's next step is usually to open the
// download on whatever machine is in front of them, and every desktop platform
// unpacks a .zip by double-clicking it while several need a separate tool for
// a .tar.
//
// Entries are stored, not deflated. The playground caps a project at 4 MiB of
// source, so compression would buy little, and shipping a compressor into the
// bundle to save a few hundred kilobytes on a rare download is a bad trade
// against a page that has to install itself for offline use.

const UTF8_ENCODER = new TextEncoder();

/** Local file header, central directory header, end of central directory. */
const LOCAL_HEADER_SIGNATURE = 0x04034b50;
const CENTRAL_HEADER_SIGNATURE = 0x02014b50;
const END_OF_CENTRAL_DIRECTORY_SIGNATURE = 0x06054b50;

/** Version 2.0: the floor for a stored entry with a UTF-8 name. */
const VERSION_NEEDED = 20;
/** Bit 11 declares the name as UTF-8 rather than the legacy code page. */
const UTF8_NAME_FLAG = 0x0800;
const STORED = 0;

/**
 * 1980-01-01 00:00:00, the earliest timestamp the DOS fields can express.
 *
 * Fixed rather than current, so downloading the same result twice produces
 * byte-identical archives. A real modification time is not information the
 * reader has any use for here, and it would make the output impossible to
 * compare.
 */
const DOS_TIME = 0;
const DOS_DATE = (1 << 5) | 1;

/** Names are one 16-bit length field, far above anything the playground accepts. */
const MAX_NAME_BYTES = 0xffff;

export interface ArchiveEntry {
  path: string;
  source: string;
}

export class ZipArchiveError extends Error {
  readonly code: "name_too_long";
  readonly path: string;

  constructor(code: "name_too_long", path: string) {
    super(code);
    this.name = "ZipArchiveError";
    this.code = code;
    this.path = path;
  }
}

export function buildZip(files: readonly ArchiveEntry[]): Uint8Array<ArrayBuffer> {
  const entries = files.map((file) => {
    const name = UTF8_ENCODER.encode(file.path);
    if (name.length > MAX_NAME_BYTES) throw new ZipArchiveError("name_too_long", file.path);
    const content = UTF8_ENCODER.encode(file.source);
    return { name, content, crc: crc32(content), offset: 0 };
  });

  const localSize = entries.reduce((total, e) => total + 30 + e.name.length + e.content.length, 0);
  const centralSize = entries.reduce((total, e) => total + 46 + e.name.length, 0);
  const archive = new Uint8Array(localSize + centralSize + 22);
  const view = new DataView(archive.buffer);
  let offset = 0;

  for (const entry of entries) {
    entry.offset = offset;
    view.setUint32(offset, LOCAL_HEADER_SIGNATURE, true);
    view.setUint16(offset + 4, VERSION_NEEDED, true);
    view.setUint16(offset + 6, UTF8_NAME_FLAG, true);
    view.setUint16(offset + 8, STORED, true);
    view.setUint16(offset + 10, DOS_TIME, true);
    view.setUint16(offset + 12, DOS_DATE, true);
    view.setUint32(offset + 14, entry.crc, true);
    view.setUint32(offset + 18, entry.content.length, true);
    view.setUint32(offset + 22, entry.content.length, true);
    view.setUint16(offset + 26, entry.name.length, true);
    view.setUint16(offset + 28, 0, true);
    archive.set(entry.name, offset + 30);
    archive.set(entry.content, offset + 30 + entry.name.length);
    offset += 30 + entry.name.length + entry.content.length;
  }

  const centralStart = offset;
  for (const entry of entries) {
    view.setUint32(offset, CENTRAL_HEADER_SIGNATURE, true);
    // Version made by: 3 (Unix) in the high byte, so the permission bits below
    // are read rather than ignored as MS-DOS attributes.
    view.setUint16(offset + 4, (3 << 8) | VERSION_NEEDED, true);
    view.setUint16(offset + 6, VERSION_NEEDED, true);
    view.setUint16(offset + 8, UTF8_NAME_FLAG, true);
    view.setUint16(offset + 10, STORED, true);
    view.setUint16(offset + 12, DOS_TIME, true);
    view.setUint16(offset + 14, DOS_DATE, true);
    view.setUint32(offset + 16, entry.crc, true);
    view.setUint32(offset + 20, entry.content.length, true);
    view.setUint32(offset + 24, entry.content.length, true);
    view.setUint16(offset + 28, entry.name.length, true);
    view.setUint16(offset + 30, 0, true);
    view.setUint16(offset + 32, 0, true);
    view.setUint16(offset + 34, 0, true);
    view.setUint16(offset + 36, 0, true);
    // External attributes. 0o100644 is a regular file readable by everyone and
    // writable by its owner; it sits in the high half, which is where the Unix
    // "version made by" above tells a reader to look.
    view.setUint32(offset + 38, (0o100644 << 16) >>> 0, true);
    view.setUint32(offset + 42, entry.offset, true);
    archive.set(entry.name, offset + 46);
    offset += 46 + entry.name.length;
  }

  view.setUint32(offset, END_OF_CENTRAL_DIRECTORY_SIGNATURE, true);
  view.setUint16(offset + 4, 0, true);
  view.setUint16(offset + 6, 0, true);
  view.setUint16(offset + 8, entries.length, true);
  view.setUint16(offset + 10, entries.length, true);
  view.setUint32(offset + 12, offset - centralStart, true);
  view.setUint32(offset + 16, centralStart, true);
  view.setUint16(offset + 20, 0, true);
  return archive;
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let index = 0; index < 256; index += 1) {
    let value = index;
    for (let bit = 0; bit < 8; bit += 1) {
      value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }
    table[index] = value >>> 0;
  }
  return table;
})();

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc = CRC_TABLE[(crc ^ byte) & 0xff]! ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
