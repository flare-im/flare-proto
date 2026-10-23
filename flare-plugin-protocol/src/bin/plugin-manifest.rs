fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: plugin-manifest <path>")?;
    let bytes = std::fs::read(path)?;
    let manifest = flare_plugin_protocol::PluginManifest::from_json(&bytes)?;
    for warning in manifest.warnings() {
        eprintln!("warning: {}: {}", warning.field, warning.reason);
    }
    println!(
        "valid declaration: {}@{} ({} operations); publisher signature and artifact trust require separate verification",
        manifest.plugin_id,
        manifest.version,
        manifest.operation_ids().len()
    );
    Ok(())
}
