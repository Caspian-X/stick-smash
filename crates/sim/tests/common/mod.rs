use sim::{
    GameData,
    data::{Source, load},
};
use std::{fs, path::Path};
pub fn data() -> GameData {
    fn collect(root: &Path, dir: &Path, out: &mut Vec<Source>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                if p.file_name().unwrap() != "schema" {
                    collect(root, &p, out);
                }
            } else if p.extension().is_some_and(|e| e == "json") {
                out.push(Source {
                    path: p.strip_prefix(root).unwrap().to_string_lossy().into(),
                    text: fs::read_to_string(p).unwrap(),
                });
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut sources = vec![];
    collect(&root, &root, &mut sources);
    let (d, errors) = load(sources).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    d
}
