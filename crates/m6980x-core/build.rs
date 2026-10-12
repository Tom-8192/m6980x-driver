// Die Sprachdateien werden beim Kompilieren eingebettet (rust-i18n); bei Aenderungen neu bauen.
fn main() {
    println!("cargo:rerun-if-changed=../../locales");
}
