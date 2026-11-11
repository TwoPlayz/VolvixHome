fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/volvixhome.ico");
        // what Windows shows in the volume mixer, the media overlay and Task Manager
        res.set("FileDescription", "Volvix Home");
        res.set("ProductName", "Volvix Home");
        res.set("CompanyName", "Volvix");
        let _ = res.compile();
    }
}
