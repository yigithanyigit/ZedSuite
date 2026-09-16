const definitionFields = [
  "address", "size", "external_source", "description", "name", "dimensions",
  "data_type", "is_little_endian", "column_major", "rows_reversed",
  "correction_factor", "offset", "unit", "x_axis_address", "y_axis_address",
  "x_axis_encoding", "y_axis_encoding", "x_axis_correction", "y_axis_correction",
  "x_axis_offset", "y_axis_offset", "x_label", "y_label",
] as const;

export function calibrationDefinitionKey(definition: object): string {
  const fields = definition as Record<string, unknown>;
  return JSON.stringify(definitionFields.map((field) => fields[field] ?? null));
}

export function retainedDefinitions<T extends { external_source?: string | null }>(maps: T[], format: string): T[] {
  const replacesImported = ["A2L", "ZedSuite"].includes(format) || maps.some((map) => ["A2L", "ZedSuite"].includes(map.external_source || ""));
  return maps.filter((map) => replacesImported ? !map.external_source : map.external_source !== format);
}
