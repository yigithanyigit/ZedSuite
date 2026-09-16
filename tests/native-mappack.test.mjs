import { test } from 'node:test';
import assert from 'node:assert/strict';
import ts from 'typescript';
import { readFileSync, writeFileSync, readdirSync, mkdtempSync, rmSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const compiled = ts.transpileModule(readFileSync('src/lib/native-mappack.ts', 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.ES2022 },
}).outputText;
const { serializeNativeMappack } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`);

test('native serializer retains precision, signs, layout and Unicode', () => {
  const map = { data_type: 'Float32', column_major: true, correction_factor: 1 / 4096,
    y_axis_encoding: { data_type: 'Int16', is_little_endian: false }, y_label: '°C' };
  const pack = JSON.parse(new TextDecoder().decode(serializeNativeMappack([map], 0x780000, ['unresolved axis'])));
  assert.deepEqual(pack.maps, [map]);
  assert.deepEqual(pack.rejected, ['unresolved axis']);
  assert.equal(pack.software, 'MG1CS003/R0R9A005B');
});

test('private A2L and XDF packs round-trip through the native command on all reference binaries', {
  skip: !process.env.MG_REFERENCE_DIR,
}, () => {
  const root = process.env.MG_REFERENCE_DIR;
  const definitions = join(root, 'analysis/definitions');
  const binaryPaths = ['250', '280', '300'].map(tune => join(root, `analysis/decoded/dbj_${tune}.bin`));
  binaryPaths.push(...readdirSync(root).filter(name => name.startsWith('WBA') && name.endsWith('.bin')).map(name => join(root, name)));
  assert.equal(binaryPaths.length, 4);
  const scratch = mkdtempSync(join(tmpdir(), 'zedsuite-pack-'));
  const packPath = join(scratch, 'reference.zedsuite.json');
  const invoke = (example, args) => JSON.parse(execFileSync(`src-tauri/target/debug/examples/${example}`, args, { maxBuffer: 16 * 1024 * 1024 }).toString());
  try {
    for (const binary of binaryPaths) {
      for (const source of ['a2l', 'xdf']) {
        const file = join(definitions, `R0R9A005B_7706-000_011_005${source === 'a2l' ? '.a2l' : '.MetricUnits.xdf'}`);
        const result = invoke(`inspect_${source}`, [file, binary]);
        const maps = source === 'a2l' ? result.maps : result;
        const rejected = source === 'a2l' ? result.rejected : [];
        assert.equal(maps.length, source === 'a2l' ? 357 : 842);
        writeFileSync(packPath, serializeNativeMappack(maps, 0x780000, rejected));
        const imported = invoke('inspect_native_mappack', [packPath, binary]);
        assert.equal(imported.format, 'ZedSuite');
        assert.deepEqual(imported.maps, JSON.parse(JSON.stringify(maps.map(map => ({ ...map, external_source: 'ZedSuite' })))));
        assert.deepEqual(imported.rejected, rejected);
      }
    }
    const valid = JSON.parse(readFileSync(packPath, 'utf8'));
    for (const change of [{ version: 2 }, { software: 'wrong' }, { rom_size: 1 }]) {
      writeFileSync(packPath, JSON.stringify({ ...valid, ...change }));
      assert.throws(() => execFileSync('src-tauri/target/debug/examples/inspect_native_mappack', [packPath, binaryPaths[0]], { stdio: 'pipe' }));
    }
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});
