fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("PROTOC").is_none() {
        let protoc = protoc_bin_vendored::protoc_bin_path()?;
        // Build scripts run before Cargo starts compiling this package and
        // prost-build reads PROTOC synchronously below.
        unsafe {
            std::env::set_var("PROTOC", protoc);
        }
    }

    let compile_result = compile_protos();

    if let Err(e) = compile_result {
        let error_msg = format!("{}", e);
        if error_msg.contains("Could not find `protoc`") {
            let out_dir = std::env::var("OUT_DIR")?;
            let generated_files = ["flare.common.v1.rs"];

            let all_files_exist = generated_files
                .iter()
                .all(|file| std::path::Path::new(&out_dir).join(file).exists());

            if all_files_exist {
                println!("cargo:warning=protoc not found, using pre-generated protobuf files");
                println!(
                    "cargo:warning=If you modify proto files, install protoc: brew install protobuf"
                );
                for proto_file in PROTO_FILES {
                    println!("cargo:rerun-if-changed={}", proto_file);
                }
                return Ok(());
            } else {
                return Err(Box::new(std::io::Error::other(format!(
                    "Failed to compile protobuf files: {}. Please install protoc: brew install protobuf",
                    e
                ))));
            }
        } else {
            return Err(e);
        }
    }

    Ok(())
}

const PROTO_FILES: &[&str] = &[
    "proto/errors.proto",
    "proto/enums.proto",
    "proto/metadata.proto",
    "proto/message_content.proto",
    "proto/message.proto",
    "proto/models.proto",
    "proto/event.proto",
    "proto/event_bus_envelope.proto",
    "proto/ack.proto",
    "proto/conversation.proto",
    "proto/sync.proto",
    "proto/topic_envelope.proto",
    "proto/data.proto",
    "proto/notification.proto",
    "proto/whitepaper_schema.proto",
];

fn compile_protos() -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg = prost_build::Config::new();

    cfg.compile_protos(PROTO_FILES, &["proto"])?;
    Ok(())
}
