import { type ArchiveEntry, buildZip } from "./project/archive";

function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export function downloadSource(path: string, source: string): void {
  const name = path.split("/").at(-1) || "normfix-output.c";
  downloadBlob(new Blob([source], { type: "text/plain;charset=utf-8" }), name);
}

export function downloadProject(files: readonly ArchiveEntry[]): void {
  const archive = buildZip(files);
  downloadBlob(new Blob([archive], { type: "application/zip" }), "normfix-formatted.zip");
}
