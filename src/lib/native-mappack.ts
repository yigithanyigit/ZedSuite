export function serializeNativeMappack(maps: object[], romSize: number, rejected: string[] = []): Uint8Array {
  return new TextEncoder().encode(JSON.stringify({
    format: "ZedSuite MG1 definitions",
    version: 1,
    software: "MG1CS003/R0R9A005B",
    rom_size: romSize,
    maps,
    rejected,
  }, null, 2) + "\n");
}
