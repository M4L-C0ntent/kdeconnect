use std::env;
use std::fs::File;
use std::path::PathBuf;

fn main() {
    let out_dir: PathBuf = env::var_os("OUT_DIR").unwrap().into();
    let output_path = out_dir.join("io_github_hepp3n_kdeconnect.rs");

    let mut input = File::open("io.github.hepp3n.kdeconnect.varlink").unwrap();
    let mut output = File::create(&output_path).unwrap();

    varlink_generator::generate_with_options(
        &mut input,
        &mut output,
        &varlink_generator::GeneratorOptions {
            generate_async: true,
            ..Default::default()
        },
        false,
    )
    .unwrap();

    println!("cargo:rerun-if-changed=io.github.hepp3n.kdeconnect.varlink");
}
