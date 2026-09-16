import { test } from 'node:test';
import assert from 'node:assert/strict';
import ts from 'typescript';
import { readFileSync } from 'node:fs';

function compile(path) {
  return ts.transpileModule(readFileSync(new URL(path, import.meta.url), 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.ES2022 },
  }).outputText;
}
const codecUrl = `data:text/javascript;base64,${Buffer.from(compile('../src/lib/calibration-codec.ts')).toString('base64')}`;
const layoutCode = compile('../src/lib/map-cell-layout.ts').replace('"./calibration-codec"', JSON.stringify(codecUrl));
const { resolveMapCellLayout } = await import(`data:text/javascript;base64,${Buffer.from(layoutCode).toString('base64')}`);
const { readCalibrationCell, writeCalibrationCell } = await import(codecUrl);

test('A2L column storage preserves X columns and Y rows for reads and edits', () => {
  const map = { external_source: 'A2L', column_major: true, data_type: 'UInt16', size: 12,
    dimensions: { TwoDimensional: { rows: 3, cols: 2 } } };
  const layout = resolveMapCellLayout(map);
  const bin = Uint8Array.from([0, 11, 0, 12, 0, 13, 0, 21, 0, 22, 0, 23]);
  const displayed = Array.from({ length: 3 }, (_, row) => Array.from({ length: 2 }, (_, col) =>
    readCalibrationCell(bin, layout.cellIndex(row, col) * layout.cellBytes, map)));
  assert.deepEqual(displayed, [[11, 21], [12, 22], [13, 23]]);
  writeCalibrationCell(bin, layout.cellIndex(1, 1) * layout.cellBytes, map, 99);
  assert.deepEqual([...bin], [0, 11, 0, 12, 0, 13, 0, 21, 0, 99, 0, 23]);
  const rowLayout = resolveMapCellLayout({ ...map, column_major: false });
  assert.equal(rowLayout.cellIndex(1, 1), 3);
});

const identityCode = compile('../src/lib/calibration-definition.ts');
const { calibrationDefinitionKey } = await import(`data:text/javascript;base64,${Buffer.from(identityCode).toString('base64')}`);
test('same-address definitions cannot share a decode cache identity', () => {
  const base = { address: 100, data_type: 'UInt16', external_source: 'A2L', column_major: true };
  for (const change of [{ data_type: 'Float32' }, { external_source: 'XDF' },
    { column_major: false }, { correction_factor: 0.5 }, { x_axis_encoding: { data_type: 'Int8' } }]) {
    assert.notEqual(calibrationDefinitionKey(base), calibrationDefinitionKey({ ...base, ...change }));
  }
  assert.equal(calibrationDefinitionKey(base), calibrationDefinitionKey({ ...base }));
});

const { retainedDefinitions } = await import(`data:text/javascript;base64,${Buffer.from(identityCode).toString('base64')}`);
test('A2L switching leaves only one imported interpretation for address-based writes', () => {
  const maps = [{ external_source: 'XDF' }, { external_source: 'JSON' }, {}];
  assert.deepEqual(retainedDefinitions(maps, 'A2L'), [{}]);
  assert.deepEqual(retainedDefinitions([{ external_source: 'A2L' }, {}], 'XDF'), [{}]);
});

const { selectedMapCellAddress } = await import(`data:text/javascript;base64,${Buffer.from(layoutCode).toString('base64')}`);
test('cursor addresses undo extraction mirrors before column indexing', () => {
  const map = { address: 0x680e5c, external_source: 'A2L', column_major: true,
    data_type: 'UInt16', size: 120, dimensions: { TwoDimensional: { rows: 6, cols: 10 } } };
  assert.equal(selectedMapCellAddress(map, 2, 1, { rowsReversed: true, colsReversed: false }), 0x680e6e);
  assert.equal(selectedMapCellAddress(map, 2, 8, { rowsReversed: true, colsReversed: true }), 0x680e6e);
  assert.throws(() => selectedMapCellAddress(map, 6, 0));
});
