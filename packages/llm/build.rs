use std::path::Path;

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let models_json = manifest_dir.join("models.json");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let output_path = Path::new(&out_dir).join("generated.rs");

    println!("cargo::rerun-if-changed={}", models_json.display());

    let source = llm_codegen::generate(&models_json).unwrap_or_else(|e| {
        panic!("Codegen failed: {e}");
    });

    std::fs::write(&output_path, &source).unwrap();
}
