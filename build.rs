use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Create config template if it doesn't exist
    let out_dir = env::var("OUT_DIR").unwrap_or_else(|_| "./".to_string());
    let template_path = Path::new(&out_dir).join("../../../config.template.toml");
    
    let template = r#"# VDKIO Configuration Template
# Copy this file to 'config.toml' and fill in your actual values

# RTSP URL for testing/examples
rtsp_url = "rtsp://example.com:3000/stream"
"#;
    
    let _ = fs::write(template_path, template);
    println!("cargo:rerun-if-changed=build.rs");
}
