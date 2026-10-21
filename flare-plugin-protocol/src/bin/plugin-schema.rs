fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema = schemars::schema_for!(flare_plugin_protocol::PluginManifest);
    let content = format!("{}\n", serde_json::to_string_pretty(&schema)?);
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, content)?;
    } else {
        print!("{content}");
    }
    Ok(())
}
