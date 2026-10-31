//! Checks this package before its library is built: its shaders validate with
//! naga, and its text tables match the code (src/) and data that name their
//! keys (render-lab-check).

fn main() {
    let dir = std::fs::canonicalize(std::env::var("CARGO_MANIFEST_DIR").unwrap()).unwrap();
    render_lab_check::check_package(&dir, &dir.join("src"));
}
