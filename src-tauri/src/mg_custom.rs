use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use sha2::{Digest, Sha256};

const IMAGE_SIZE: usize = 0x780000;
const CONTAINER_SIZE: usize = IMAGE_SIZE + 40;

pub fn matches_reference_software(data: &[u8]) -> bool {
    data.len() == IMAGE_SIZE
        && data.get(0x3fe4c..0x3fe54) == Some(b"MG1CS003")
        && data.get(0x67fe2d..0x67fe36) == Some(b"R0R9A005B")
        && data.get(0x77fe52..0x77fe58) == Some(b"DME8.4")
}

pub fn decrypt(data: &[u8], vin: &str) -> Result<Vec<u8>, String> {
    if vin.len() != 17 || !vin.bytes().all(|b| b.is_ascii_digit() || (b.is_ascii_uppercase() && !b"IOQ".contains(&b))) {
        return Err("Enter the 17-character VIN used for this custom file.".into());
    }
    if data.len() != CONTAINER_SIZE || data[..4] != 100_000_u32.to_be_bytes() || data[4..8] != 16_u32.to_be_bytes() {
        return Err("Unsupported custom container. Only the verified 7.5 MiB format is supported.".into());
    }
    let mut derived = [0_u8; 32];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(vin.as_bytes(), &data[8..24], 100_000, &mut derived);
    let decoded = cbc::Decryptor::<aes::Aes128>::new_from_slices(&derived[..16], &derived[16..])
        .map_err(|_| "Invalid cipher parameters")?
        .decrypt_padded_vec_mut::<Pkcs7>(&data[24..])
        .map_err(|_| "Decryption failed: wrong VIN or damaged container.")?;
    if !matches_reference_software(&decoded) {
        return Err("Decoded image does not match the supported MG1 reference software.".into());
    }
    Ok(decoded)
}

#[derive(Serialize)]
pub struct DecodedCustom {
    data_base64: String,
    sha256: String,
    bytes: usize,
}

#[tauri::command]
pub fn decode_mg_custom(file_data_base64: String, vin: String) -> Result<DecodedCustom, String> {
    if file_data_base64.len() > CONTAINER_SIZE.div_ceil(3) * 4 {
        return Err("Custom container is too large.".into());
    }
    let data = STANDARD.decode(file_data_base64).map_err(|_| "Invalid file encoding")?;
    let decoded = decrypt(&data, &vin)?;
    Ok(DecodedCustom {
        sha256: format!("{:x}", Sha256::digest(&decoded)),
        bytes: decoded.len(),
        data_base64: STANDARD.encode(decoded),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::BlockEncryptMut;

    const VIN: &str = "TEST1234567890123";

    fn fixture() -> (Vec<u8>, Vec<u8>) {
        let mut image: Vec<u8> = (0..IMAGE_SIZE).map(|i| (i % 251) as u8).collect();
        image[0x3fe4c..0x3fe54].copy_from_slice(b"MG1CS003");
        image[0x67fe2d..0x67fe36].copy_from_slice(b"R0R9A005B");
        image[0x77fe52..0x77fe58].copy_from_slice(b"DME8.4");
        let salt = [7_u8; 16];
        let mut derived = [0_u8; 32];
        pbkdf2::pbkdf2_hmac::<sha1::Sha1>(VIN.as_bytes(), &salt, 100_000, &mut derived);
        let mut container = Vec::from(100_000_u32.to_be_bytes());
        container.extend(16_u32.to_be_bytes());
        container.extend(salt);
        container.extend(cbc::Encryptor::<aes::Aes128>::new_from_slices(&derived[..16], &derived[16..]).unwrap().encrypt_padded_vec_mut::<Pkcs7>(&image));
        (container, image)
    }

    #[test]
    fn round_trip_and_wrong_vin() {
        let (data, image) = fixture();
        assert_eq!(decrypt(&data, VIN).unwrap(), image);
        assert!(decrypt(&data, "TEST1234567890124").is_err());
    }

    #[test]
    fn rejects_invalid_header_and_truncation() {
        assert!(decrypt(&[], VIN).is_err());
        assert!(decrypt(&vec![0; CONTAINER_SIZE], VIN).is_err());
        assert!(decrypt(&[], "bad").is_err());
    }

    #[test]
    #[ignore = "requires private local fixtures via MG_REFERENCE_DIR"]
    fn matches_private_reference_bins() {
        let root = std::path::PathBuf::from(std::env::var("MG_REFERENCE_DIR").expect("MG_REFERENCE_DIR"));
        let stock = std::fs::read_dir(&root).unwrap().filter_map(Result::ok).map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "bin") && p.file_name().unwrap().to_string_lossy().contains("_btld_")).expect("stock BIN");
        assert!(matches_reference_software(&std::fs::read(&stock).unwrap()));
        let name = stock.file_name().unwrap().to_string_lossy();
        let vin = name.split('_').next().unwrap();
        for config in [250, 280, 300] {
            let data = std::fs::read(root.join(format!("dbj_{config}.custom"))).unwrap();
            let expected = std::fs::read(root.join(format!("analysis/decoded/dbj_{config}.bin"))).unwrap();
            assert!(matches_reference_software(&expected));
            assert_eq!(decrypt(&data, vin).unwrap(), expected, "config {config}");
        }
    }
}
