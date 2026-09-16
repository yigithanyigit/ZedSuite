fn main() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "usage: inspect_native_mappack definitions.json binary.bin"
    );
    let data = std::fs::read(&args[1]).expect("read definitions");
    let binary = std::fs::read(&args[2]).expect("read binary");
    let result = zedsuite_lib::commands::import_map_definitions(
        STANDARD.encode(&data),
        "definitions.json".into(),
        binary.len() as u32,
        Some(STANDARD.encode(&binary)),
    )
    .expect("import definitions");
    println!("{}", serde_json::to_string(&result).unwrap());
}
