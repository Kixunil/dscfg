extern crate configure_me_codegen;

fn main() {
    configure_me_codegen::build_script("config.toml").unwrap();
}
