fn main() {
    // Only embed icon on Windows
    #[cfg(windows)]
    {
        use std::env;
        use std::path::PathBuf;

        let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        let icon_path = PathBuf::from(manifest_dir)
            .join("../../../assets/icon.ico")
            .canonicalize()
            .expect("Failed to resolve icon path");

        let mut res = winres::WindowsResource::new();
        res.set_icon(icon_path.to_str().expect("Invalid icon path"));
        res.compile().expect("Failed to compile Windows resources");
    }
}
