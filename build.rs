use std::{
    env, fs,
    path::{Path, PathBuf},
};
fn walk(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("web assets").flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, files)
        } else if path.is_file() {
            files.push(path.strip_prefix(root).unwrap().to_owned());
        }
    }
}
fn main() {
    println!("cargo:rerun-if-changed=web");
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("web");
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    files.sort();
    let mut code =
        String::from("fn bundled_asset(path: &str) -> Option<&'static [u8]> { match path {\n");
    for file in files {
        code.push_str(&format!(
            "{:?} => Some(include_bytes!({:?})),\n",
            file.to_string_lossy(),
            root.join(&file).to_string_lossy()
        ));
    }
    code.push_str("_ => None, } }\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("web_assets.rs"),
        code,
    )
    .unwrap();
}
