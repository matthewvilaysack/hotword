use hotword::workflow::Workflow;

#[test]
fn every_example_parses_and_validates() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let wf = Workflow::from_toml(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            format!("{}.toml", wf.name),
            path.file_name().unwrap().to_string_lossy()
        );
        seen += 1;
    }
    assert_eq!(seen, 4);
}
