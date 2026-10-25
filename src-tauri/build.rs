use std::process::Command;

fn main() -> Result<(), String> {
    let target = std::env::var("TARGET").map_err(|error| format!("TARGET: {error}"))?;
    println!("cargo:rerun-if-changed=../scripts/build-llama-server.sh");
    println!("cargo:rerun-if-changed=binaries/llama-server-{target}");
    let built = Command::new("bash")
        .arg("../scripts/build-llama-server.sh")
        .arg(&target)
        .status()
        .map_err(|error| format!("scripts/build-llama-server.sh did not run: {error}"))?;
    if !built.success() {
        return Err(format!("scripts/build-llama-server.sh failed: {built}"));
    }
    tauri_build::build();
    Ok(())
}
