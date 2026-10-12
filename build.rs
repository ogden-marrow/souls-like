fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Souls-like");
        res.set("FileDescription", "Souls-like launcher and save manager");
        res.set("CompanyName", "Souls-like contributors");
        res.set("LegalCopyright", "MIT License");
        res.set("OriginalFilename", "souls-like.exe");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
