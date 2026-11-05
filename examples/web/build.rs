fn main() {
    // Catalog files are shared with the native example in the root package.
    // Recognize its cfg name without exposing/enabling desktop features here.
    println!("cargo:rustc-check-cfg=cfg(feature, values(\"native\"))");
}
