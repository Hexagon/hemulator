fn main() {
    // Only embed icon on Windows
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../../assets/icon.ico");
        res.compile().expect("Failed to compile Windows resources");
    }
}
