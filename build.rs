fn main() {
    println!("cargo:rerun-if-changed=assets/windows/MarkRush.rc");
    println!("cargo:rerun-if-changed=assets/icons/MarkRush.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/windows/MarkRush.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("failed to embed Windows app icon");
    }
}
