use llm::catalog::codegen;
use std::path::Path;

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let models_json = manifest_dir.join("models.json");
    let output_path = manifest_dir.join("src/catalog/generated.rs");

    eprintln!("Reading {}", models_json.display());
    let source = codegen::generate(&models_json).unwrap_or_else(|e| {
        eprintln!("Codegen failed: {e}");
        std::process::exit(1);
    });

    std::fs::write(&output_path, &source).unwrap_or_else(|e| {
        eprintln!("Failed to write {}: {e}", output_path.display());
        std::process::exit(1);
    });

    eprintln!("Generated {}", output_path.display());
}
