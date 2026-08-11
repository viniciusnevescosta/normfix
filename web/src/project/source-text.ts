const UTF8_DECODER = new TextDecoder("utf-8", {
  fatal: true,
  // File decoders conventionally consume a leading UTF-8 signature. Keeping
  // U+FEFF here would move it behind a newly inserted 42 header.
  ignoreBOM: false,
});

export function decodeUtf8Source(bytes: ArrayBuffer): string {
  return UTF8_DECODER.decode(bytes);
}
