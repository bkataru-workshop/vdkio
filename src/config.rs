use lazy_static::lazy_static;
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::RwLock;

lazy_static! {
    static ref CONFIG: RwLock<Config> = RwLock::new(Config::new());
}

#[derive(Debug, Clone)]
pub struct Config {
    pub rtsp_url: String,
}

impl Config {
    fn new() -> Self {
        // Default values (not containing sensitive information)
        let mut config = Config {
            rtsp_url: String::from("rtsp://example.com:3000/stream"),
        };

        // Try loading from environment variables first
        if let Ok(url) = env::var("VDKIO_RTSP_URL") {
            config.rtsp_url = url;
        }

        // Then try loading from config file
        let config_paths = ["./config.toml", "./vdkio_config.toml"];
        for path in &config_paths {
            if let Ok(mut file) = File::open(path) {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    if let Some(line) = content
                        .lines()
                        .find(|line| line.starts_with("rtsp_url"))
                    {
                        if let Some(url) = line.split('=').nth(1) {
                            let url = url.trim().trim_matches('"').trim_matches('\'');
                            if !url.is_empty() {
                                config.rtsp_url = url.to_string();
                            }
                        }
                    }
                }
            }
        }

        config
    }

    pub fn reload() {
        let new_config = Config::new();
        if let Ok(mut config) = CONFIG.write() {
            *config = new_config;
        }
    }
}

/// Returns the RTSP URL from configuration
pub fn get_rtsp_url() -> String {
    CONFIG.read().unwrap().rtsp_url.clone()
}

/// Creates a default config template file if it doesn't exist
pub fn create_default_config_template<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    if !path.as_ref().exists() {
        let template = r#"# VDKIO Configuration
# This is a template. Replace the values with your actual configuration.

# RTSP URL for testing/examples
rtsp_url = "rtsp://example.com:3000/stream"
"#;
        std::fs::write(path, template)?;
    }
    Ok(())
}
