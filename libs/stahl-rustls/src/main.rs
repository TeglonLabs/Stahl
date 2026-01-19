use stahl_rustls::build_module;

fn main() {
    std::process::Command::new("cargo-stahl-lib")
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    build_module()
        .emit_package_to_file("libstahl_rustls", "rustls.scm")
        .unwrap()
}
