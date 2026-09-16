export interface CalibrationEncoding {
  data_type?: string;
  is_little_endian?: boolean;
}

export function calibrationCellBytes(type?: string): number {
  switch (type) {
    case "UInt8": case "Int8": return 1;
    case "UInt16": case "Int16": return 2;
    case "UInt32": case "Int32": case "Float32": return 4;
    default: throw new Error(`Unsupported calibration type: ${type}`);
  }
}

export function readCalibrationCell(data: ArrayLike<number>, address: number, encoding: CalibrationEncoding): number {
  const size = calibrationCellBytes(encoding.data_type);
  if (!Number.isInteger(address) || address < 0 || address + size > data.length) throw new Error("Calibration address outside binary");
  const bytes = Uint8Array.from({ length: size }, (_, i) => data[address + i]);
  const view = new DataView(bytes.buffer);
  const le = encoding.is_little_endian === true;
  switch (encoding.data_type) {
    case "UInt8": return view.getUint8(0);
    case "Int8": return view.getInt8(0);
    case "UInt16": return view.getUint16(0, le);
    case "Int16": return view.getInt16(0, le);
    case "UInt32": return view.getUint32(0, le);
    case "Int32": return view.getInt32(0, le);
    case "Float32": return view.getFloat32(0, le);
    default: throw new Error("Unsupported calibration type");
  }
}

export function writeCalibrationCell(data: Uint8Array, address: number, encoding: CalibrationEncoding, value: number, factor = 1, offset = 0): void {
  const size = calibrationCellBytes(encoding.data_type);
  if (!Number.isInteger(address) || address < 0 || address + size > data.length) throw new Error("Calibration address outside binary");
  if (![value, factor, offset].every(Number.isFinite) || factor === 0) throw new Error("Invalid calibration conversion");
  let raw = (value - offset) / factor;
  if (encoding.data_type !== "Float32") {
    raw = Math.round(raw);
    const signed = encoding.data_type?.startsWith("Int");
    const min = signed ? -(2 ** (size * 8 - 1)) : 0;
    const max = signed ? 2 ** (size * 8 - 1) - 1 : 2 ** (size * 8) - 1;
    if (raw < min || raw > max) throw new Error("Calibration value outside storage range");
  } else if (!Number.isFinite(Math.fround(raw))) throw new Error("Calibration float overflow");
  const view = new DataView(data.buffer, data.byteOffset + address, size);
  const le = encoding.is_little_endian === true;
  switch (encoding.data_type) {
    case "UInt8": view.setUint8(0, raw); break;
    case "Int8": view.setInt8(0, raw); break;
    case "UInt16": view.setUint16(0, raw, le); break;
    case "Int16": view.setInt16(0, raw, le); break;
    case "UInt32": view.setUint32(0, raw, le); break;
    case "Int32": view.setInt32(0, raw, le); break;
    case "Float32": view.setFloat32(0, raw, le); break;
  }
}
