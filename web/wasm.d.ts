declare module "*normfix_wasm.js" {
  export default function initialize(): Promise<unknown>;
  export function formatProject(request: string): string;
}
