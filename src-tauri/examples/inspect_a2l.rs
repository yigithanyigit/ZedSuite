fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        3,
        "usage: inspect_a2l definitions.a2l binary.bin"
    );
    let data = std::fs::read(&args[1]).expect("read A2L");
    let binary = std::fs::read(&args[2]).expect("read binary");
    let result = zedsuite_lib::a2l_import::parse_reference(&data, &binary).expect("import A2L");
    println!("{}", serde_json::to_string(&result).unwrap());
}
