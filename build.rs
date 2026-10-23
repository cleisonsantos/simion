#[cfg(windows)]
fn embed_resources() {
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/branding/simion.ico");
    resource.set("ProductName", "Simion");
    resource.set("FileDescription", "Simion — caderno de blocos local");
    resource.set("LegalCopyright", "MIT");
    resource.set("OriginalFilename", "simion.exe");
    resource
        .compile()
        .expect("não foi possível embutir os recursos do Windows");
}

fn main() {
    println!("cargo:rerun-if-changed=assets/branding/simion.ico");

    #[cfg(windows)]
    embed_resources();
}
