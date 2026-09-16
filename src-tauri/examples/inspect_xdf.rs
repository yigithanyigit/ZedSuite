fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 3, "usage: inspect_xdf definitions.xdf binary.bin");
    let data = std::fs::read(&args[1]).expect("read XDF");
    let xml = zedsuite_lib::xdf_import::decode_text(&data);
    let binary = std::fs::read(&args[2]).expect("read binary");
    let maps = zedsuite_lib::xdf_import::parse_xdf(&xml, binary.len() as u32);
    println!("{}", serde_json::to_string(&maps).unwrap());
}
