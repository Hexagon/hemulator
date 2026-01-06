fn main() {
    // Only embed icon on Windows
    #[cfg(windows)]
    {
        use std::env;
        use std::path::PathBuf;

        let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect(
            "CARGO_MANIFEST_DIR environment variable not set. \
             This should be set automatically by Cargo during build.",
        );
        let icon_path = PathBuf::from(&manifest_dir).join("../../../assets/icon.ico");

        let icon_path = icon_path.canonicalize().unwrap_or_else(|e| {
            panic!(
                "Failed to resolve icon path: {:?}\n\
                 Attempted path: {:?}\n\
                 Error: {}",
                icon_path,
                icon_path.display(),
                e
            )
        });

        let mut res = winres::WindowsResource::new();
        res.set_icon(icon_path.to_str().expect("Invalid icon path"));
        res.compile().expect("Failed to compile Windows resources");
    }
}
