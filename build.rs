use std::path::Path;

use spirv_builder::SpirvBuilder;

fn build_shader(path_to_crate: &str) {
    let path_to_crate = Path::new(env!("CARGO_MANIFEST_DIR")).join(path_to_crate);

    let mut builder = SpirvBuilder::new(path_to_crate, "spirv-unknown-vulkan1.1");
    builder.build_script.defaults = true;
    builder.build_script.env_shader_spv_path = Some(true);
    builder.build().expect("Kernel failed to compile");
}

fn main() {
    build_shader("shaders");
}
