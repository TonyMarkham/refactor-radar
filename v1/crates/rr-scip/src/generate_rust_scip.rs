use crate::{ScipError, ScipResult};

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};
use tokio::process::Command;

pub async fn generate_rust_scip(
    project_path: &Path,
    output_path: &Path,
    rust_analyzer_path: Option<&Path>,
    config_path: Option<&Path>,
    exclude_vendored_libraries: bool,
    num_threads: Option<usize>,
) -> ScipResult<String> {
    let executable = rust_analyzer_path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("rust-analyzer"));
    let mut command = Command::new(&executable);
    command
        .arg("scip")
        .arg(project_path)
        .arg("--output")
        .arg(output_path);
    if let Some(config_path) = config_path {
        command.arg("--config-path").arg(config_path);
    }
    if exclude_vendored_libraries {
        command.arg("--exclude-vendored-libraries");
    }
    if let Some(num_threads) = num_threads {
        command.arg("--num-threads").arg(num_threads.to_string());
    }
    let output = command.output().await.map_err(|error| match error.kind() {
        ErrorKind::NotFound => ScipError::rust_analyzer_missing(executable.clone()),
        _ => ScipError::rust_analyzer_launch_failed(executable.clone(), error.to_string()),
    })?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        return Err(ScipError::rust_analyzer_failed(
            output.status.code(),
            stderr,
        ));
    }
    Ok(stderr)
}
