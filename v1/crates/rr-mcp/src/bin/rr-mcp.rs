use rr_mcp::{McpError, McpResult, McpServer};

use clap::Parser;
use rmcp::{ServiceExt, transport::stdio};
use std::{path::PathBuf, process::ExitCode};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    rust_analyzer_path: Option<PathBuf>,

    #[arg(long)]
    brief_model: Option<String>,

    #[arg(long)]
    max_concurrent_requests: Option<usize>,
}

#[tokio::main]
async fn main() -> ExitCode {
    // Process entrypoints must report OS success/failure, and MCP stdio
    // reserves stdout for protocol messages. Keep typed errors in run(), then
    // print startup/runtime failures only to stderr before returning ExitCode.
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to run RefactorRadar MCP server: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> McpResult<()> {
    let args = Args::parse();

    let service = McpServer::with_runtime_config(
        args.rust_analyzer_path,
        args.brief_model,
        args.max_concurrent_requests,
    )
    .serve(stdio())
    .await
    .map_err(|error| McpError::server_runtime(error.to_string()))?;

    service
        .waiting()
        .await
        .map_err(|error| McpError::server_runtime(error.to_string()))?;

    Ok(())
}
