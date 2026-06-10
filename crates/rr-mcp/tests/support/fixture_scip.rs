use protobuf::{Message, MessageField};
use scip::types::{
    Document, Index, Metadata, Occurrence, SymbolInformation, SymbolRole, ToolInfo,
    symbol_information,
};
use tempfile::tempdir;

pub const PROJECT_ID: &str = "fixture";
pub const SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().";

pub fn write_fixture_scip()
-> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(&path, sample_index().write_to_bytes()?)?;
    Ok((directory, path))
}

fn sample_index() -> Index {
    Index {
        metadata: MessageField::some(Metadata {
            project_root: PROJECT_ID.to_owned(),
            tool_info: MessageField::some(ToolInfo {
                name: "rust-analyzer".to_owned(),
                version: "test".to_owned(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        documents: vec![Document {
            relative_path: "src/lib.rs".to_owned(),
            language: "rust".to_owned(),
            symbols: vec![SymbolInformation {
                symbol: SYMBOL.to_owned(),
                kind: symbol_information::Kind::Function.into(),
                display_name: "public_sum".to_owned(),
                ..Default::default()
            }],
            occurrences: vec![
                Occurrence {
                    range: vec![0, 0, 10],
                    symbol: SYMBOL.to_owned(),
                    symbol_roles: SymbolRole::Definition as i32,
                    ..Default::default()
                },
                Occurrence {
                    range: vec![1, 0, 10],
                    symbol: SYMBOL.to_owned(),
                    symbol_roles: 0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    }
}
