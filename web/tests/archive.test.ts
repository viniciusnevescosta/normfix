import assert from "node:assert/strict";
import { test } from "vitest";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { ZipArchiveError, buildZip } from "../src/project/archive";

const decoder = new TextDecoder();

function u16(archive: Uint8Array, offset: number): number {
  return new DataView(archive.buffer, archive.byteOffset).getUint16(offset, true);
}

function u32(archive: Uint8Array, offset: number): number {
  return new DataView(archive.buffer, archive.byteOffset).getUint32(offset, true);
}

test("the archive opens as a zip and carries the file back out", () => {
  const archive = buildZip([{ path: "src/main.c", source: "int x;\n" }]);

  assert.equal(u32(archive, 0), 0x04034b50);
  assert.equal(u16(archive, 8), 0, "stored, not deflated");
  assert.equal(decoder.decode(archive.slice(30, 40)), "src/main.c");
  assert.equal(decoder.decode(archive.slice(40, 47)), "int x;\n");
  assert.equal(u32(archive, 18), 7, "compressed size");
  assert.equal(u32(archive, 22), 7, "uncompressed size");
});

test("the end of central directory points at every entry", () => {
  const archive = buildZip([
    { path: "a.c", source: "int a;\n" },
    { path: "b.h", source: "int b;\n" },
  ]);

  const end = archive.length - 22;
  assert.equal(u32(archive, end), 0x06054b50);
  assert.equal(u16(archive, end + 8), 2, "entries on this disk");
  assert.equal(u16(archive, end + 10), 2, "entries in total");

  const centralStart = u32(archive, end + 16);
  assert.equal(u32(archive, centralStart), 0x02014b50);
  assert.equal(u32(archive, end + 12), end - centralStart, "central directory size");

  // Each central header must point back at a real local header, or a reader
  // that trusts the directory — which is every reader — extracts garbage.
  let offset = centralStart;
  for (const name of ["a.c", "b.h"]) {
    const nameLength = u16(archive, offset + 28);
    assert.equal(decoder.decode(archive.slice(offset + 46, offset + 46 + nameLength)), name);
    assert.equal(u32(archive, u32(archive, offset + 42)), 0x04034b50);
    offset += 46 + nameLength;
  }
});

test("names are declared as UTF-8 rather than left to a legacy code page", () => {
  const archive = buildZip([{ path: "src/açaí.c", source: "" }]);

  assert.equal(u16(archive, 6) & 0x0800, 0x0800);
  const nameLength = u16(archive, 26);
  assert.equal(decoder.decode(archive.slice(30, 30 + nameLength)), "src/açaí.c");
});

test("the same result downloads to the same bytes twice", () => {
  // Timestamps are fixed rather than current, so two downloads of one result
  // can be compared, and a re-download is not a spurious diff.
  const files = [{ path: "main.c", source: "int main(void)\n{\n}\n" }];

  assert.deepEqual(buildZip(files), buildZip(files));
});

test("an empty project still produces a readable archive", () => {
  const archive = buildZip([]);

  assert.equal(archive.length, 22);
  assert.equal(u32(archive, 0), 0x06054b50);
  assert.equal(u16(archive, 8), 0);
});

test("a name too long for the zip name field is refused, not truncated", () => {
  const path = `${"a".repeat(70000)}.c`;

  assert.throws(
    () => buildZip([{ path, source: "" }]),
    (error: unknown) => {
      assert.ok(error instanceof ZipArchiveError);
      assert.equal(error.code, "name_too_long");
      assert.equal(error.path, path);
      return true;
    },
  );
});

test("a real unzip implementation accepts the archive", (t) => {
  // The point of choosing zip over tar was that a student can open it with
  // whatever is already on their machine. Checking the bytes against the spec
  // is not the same as checking that a reader agrees.
  const directory = mkdtempSync(join(tmpdir(), "normfix-zip-"));
  try {
    const archive = buildZip([
      { path: "src/main.c", source: "int main(void)\n{\n\treturn (0);\n}\n" },
      { path: "Makefile", source: "NAME = app\n" },
      { path: "src/açaí.h", source: "#ifndef A\n#endif\n" },
    ]);
    const zipPath = join(directory, "formatted.zip");
    writeFileSync(zipPath, archive);

    try {
      execFileSync("unzip", ["-qq", zipPath, "-d", directory], { stdio: "pipe" });
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") {
        t.skip("unzip is not installed on this machine");
        return;
      }
      throw error;
    }

    assert.equal(
      readFileSync(join(directory, "src/main.c"), "utf8"),
      "int main(void)\n{\n\treturn (0);\n}\n",
    );
    assert.equal(readFileSync(join(directory, "Makefile"), "utf8"), "NAME = app\n");
    assert.equal(readFileSync(join(directory, "src/açaí.h"), "utf8"), "#ifndef A\n#endif\n");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
