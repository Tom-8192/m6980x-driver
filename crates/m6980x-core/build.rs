// The language files are embedded at compile time (rust-i18n); rebuild when they change.
fn main() {
    println!("cargo:rerun-if-changed=../../locales");
}
