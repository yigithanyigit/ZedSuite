# MG1 reference workflow on macOS

This fork adds native Rust import of the verified MG Flasher `.custom`
container and explicit XDF cell/axis encoding in the editor. It is an offline
editor, not an ECU simulator or a validated MG1 flashing tool.

## Supported reference

The current identification gate is deliberately limited to the 0x780000-byte
MG1CS003 / DME8.4 image carrying R0R9A005B at the reference software offset.
It was checked against the supplied stock, 250, 280 and 300 binaries. Other
software versions are not claimed to be supported.

The `.custom` reader accepts the observed header (100000 PBKDF2-HMAC-SHA1
iterations, 16-byte salt), derives the AES-128-CBC key/IV from the entered VIN,
and verifies PKCS7 padding, image size and reference software markers. The VIN
is not saved. This format has no authenticated integrity tag; successful
import is not proof of engine safety or flash integrity. Export is raw BIN,
not a repackaged `.custom` file.

## Definitions

The user originally supplied binaries, not an XDF. The existing local XDF
and A2L were independently matched to MG Flasher's official repository at:

https://github.com/mgflasher-team/mgflasher-map-packs/tree/769f6974fcf9587e40a55faa5519db2f2bd072b1

- XDF: `R0R9A005B_7706-000_011_005.MetricUnits.xdf`
  Git blob `60ffff9a0c3d26635a8a83f13542750cdc8366a5`.
- A2L: `R0R9A005B_7706-000_011_005.a2l`
  Git blob `5583106f1f993dd23992ae6a8064654522833c80`.

These are partial definitions. The XDF contains 718 tables and 124 constants;
842 imports were checked for matching addresses and axis encodings. These are
entries, including repeated axis definitions, not 842 independently verified
ECU functions. Faithful XDF import does not validate the author's units,
conversion factors, map semantics or applicability to custom-code overlays.
Previously identified XDF/A2L discrepancies still require review. This fork
does not yet import A2L directly.

Open a BIN, or open a `.custom` file and enter its VIN. Create the project,
then choose **Import map definitions** and select the matching XDF. Search
by title, symbol (description), or hexadecimal address. Reimport definitions
in projects created with older builds to obtain the corrected metadata.

## Validation

- Native ARM64 macOS app built and launched without Wine.
- All three private custom containers decrypt byte-for-byte identically to
  the separately decoded reference BINs.
- Native UI opened the 250 custom file, imported the XDF and displayed
  `KF_LABAS_1` with its RPM/load axes.
- UI export without edits exactly matched the original 250 SHA-256.
- Disposable UI edit at displayed 6950 RPM / 180% load changed only the two
  expected bytes at 0x6BC182, from 0x0CF5 to 0x0E00 (lambda 0.875).
  This was an editor test, not a proposed calibration. The value was restored.
- 56 standard Rust tests passed, plus the private reference test; TypeScript
  checking and both codec tests passed.
- Unit tests cover integer signs, byte order, 32-bit and floating-point data,
  scaled writes, bounds, overflow and unsupported XDF layouts/conversions.

No MG1 checksum, signature, recovery or vehicle-flashing path is validated.
The app must not describe exports as flash-ready. Physical engine behavior
and power cannot be established by the editor or a table-only simulation.

## Development

```sh
npm ci
cargo test --manifest-path src-tauri/Cargo.toml --lib
node --test tests/calibration-codec.test.mjs
npx tsc --noEmit
npm run tauri -- build --debug --bundles app
```

Private fixtures stay outside the repository:

```sh
MG_REFERENCE_DIR=/path/to/remaps cargo test --manifest-path src-tauri/Cargo.toml \
  --lib matches_private_reference_bins -- --ignored
```

The fixture directory contains `dbj_{250,280,300}.custom`, the stock
`*_btld_*.bin` (VIN supplied through its filename), and the separately decoded
`analysis/decoded/dbj_{250,280,300}.bin` files. No fixtures are redistributed.

The fork uses its own macOS bundle ID and checks releases from this fork.
Local builds are ad-hoc signed, not notarized distribution releases.

## Related projects reviewed

- ZedSuite PR #13 adds Mercedes diesel EDC16CP31. Its strict identification
  and private-fixture testing are useful examples; its detector/checksum
  implementation does not apply to MG1CS003.
- StageX's MG1CS003 listing shows map families including lambda, knock,
  ignition corrections, gearbox protection and wastegate PID. It does not
  publicly establish addresses or coverage for R0R9A005B.
- NitroData's MG1 page mixes petrol ECU claims with diesel DPF/AdBlue services;
  it is not used as calibration evidence.

The linked Cartelematics DME_8C0 listing describes a BMW 760Li V12, not this
B48/DME8.4 reference. Matching the MG1CS003 family name alone is insufficient.

## Follow-up audit

A comparison of 734 numeric parts covered by the existing hash-pinned
A2L/XDF workbench found 116 parts with metadata discrepancies after fixing
constant-unit import: 100 unit-text differences, 15 conversion differences,
and three storage-type disagreements (categories overlap). Unit-text
inequality includes notation differences and is not automatically a physical
unit error. The three `EngDa_*` constants are declared float32 by the A2L but
signed int32 by the XDF. They must not be treated as verified editable
calibrations until the definitions are reconciled.

Version reconstruction now propagates failures to a visible comparison error
instead of returning partial data or attempting a raw-value fallback. Copying
imported maps uses their explicit storage encoding, including signed and
32-bit values. The audit script/report remain in the local analysis workspace;
vehicle data and definition files are not redistributed with the fork.
