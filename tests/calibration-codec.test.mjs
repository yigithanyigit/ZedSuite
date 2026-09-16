import { test } from 'node:test';
import assert from 'node:assert/strict';
import ts from 'typescript';
import { readFileSync } from 'node:fs';
const source = readFileSync(new URL('../src/lib/calibration-codec.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ES2022 } }).outputText;
const { readCalibrationCell: read, writeCalibrationCell: write } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

test('known big and little endian, signed and floating point bytes', () => {
  for (const [data_type, bytes, expected] of [
    ['UInt16', [0x12, 0x34], 4660], ['Int16', [0xff, 0xfe], -2],
    ['UInt32', [0xfe, 0xdc, 0xba, 0x98], 4275878552],
    ['Int32', [0xff, 0xff, 0xff, 0xfe], -2],
    ['Float32', [0x3f, 0x40, 0, 0], 0.75], ['Int8', [0xfe], -2],
  ]) {
    assert.equal(read(bytes, 0, { data_type }), expected);
    assert.equal(read([...bytes].reverse(), 0, { data_type, is_little_endian: true }), expected);
    const output = new Uint8Array(bytes.length + 2).fill(0xaa);
    write(output, 1, { data_type }, expected);
    assert.deepEqual([...output], [0xaa, ...bytes, 0xaa]);
  }
});

test('scaled edit changes only the intended cell and rejects overflow', () => {
  const bytes = Uint8Array.from([1, 2, 0x10, 0, 5, 6]);
  const encoding = { data_type: 'UInt16' };
  write(bytes, 2, encoding, 0.875, 1 / 4096);
  assert.deepEqual([...bytes], [1, 2, 0x0e, 0, 5, 6]);
  assert.equal(read(bytes, 2, encoding) / 4096, 0.875);
  const unchanged = bytes.slice();
  for (const value of [-1, 65536, NaN, Infinity]) assert.throws(() => write(bytes, 2, encoding, value));
  assert.throws(() => write(bytes, 2, encoding, 1, 0));
  assert.throws(() => write(bytes, 5, encoding, 1));
  assert.deepEqual(bytes, unchanged);
});
