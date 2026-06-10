use protobuf::{Message, MessageField};
use rr_scip::{Format, ScipError, generate_rust_scip, project_scip_index};
use scip::types::{
    Document, Index, Metadata, MultiLineRange, Occurrence, Signature, SingleLineRange,
    SymbolInformation, SymbolRole, ToolInfo, symbol_information,
};
use tempfile::tempdir;

const SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().";
const PRIVATE_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/private_offset().";
const PARAMETER_SYMBOL: &str = "local 0";
const PARAMETER_TYPE_SYMBOL: &str = "rust-analyzer cargo other 0.0.0 primitive/i32#";
const RETURN_SYMBOL: &str = "rust-analyzer cargo core 0.0.0 primitive/i32#";

#[test]
fn given_binary_scip_when_projecting_then_symbols_and_ranges_are_preserved()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(&path, sample_index().write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    assert_eq!("fixture", model.project.project_id);
    assert_eq!(1, model.files.len());
    assert_eq!(1, model.elements.len());
    assert_eq!(
        Some(SYMBOL),
        model
            .elements
            .first()
            .map(|element| element.scip_symbol.as_str())
    );
    assert_eq!(Some(1), model.spans.first().map(|span| span.start_line));
    Ok(())
}

#[test]
fn given_typed_scip_range_when_projecting_then_typed_range_takes_precedence()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    let mut index = sample_index();
    if let Some(document) = index.documents.first_mut()
        && let Some(occurrence) = document.occurrences.first_mut()
    {
        occurrence.range = vec![9, 9, 12];
        occurrence.set_single_line_range(SingleLineRange {
            line: 2,
            start_character: 4,
            end_character: 9,
            ..Default::default()
        });
    }
    std::fs::write(&path, index.write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    assert_eq!(Some(3), model.spans.first().map(|span| span.start_line));
    assert_eq!(Some(5), model.spans.first().map(|span| span.start_column));
    Ok(())
}

#[test]
fn given_json_scip_when_projecting_then_explicit_format_reads_protobuf_json()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.json");
    let json = protobuf_json_mapping::print_to_string(&sample_index())?;
    std::fs::write(&path, json)?;

    let model = project_scip_index(&path, Some(Format::Json))?;

    assert_eq!(1, model.files.len());
    assert_eq!(1, model.elements.len());
    Ok(())
}

#[test]
fn given_json_extension_when_format_is_not_explicit_then_format_is_detected()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.json");
    let json = protobuf_json_mapping::print_to_string(&sample_index())?;
    std::fs::write(&path, json)?;

    let model = project_scip_index(&path, None)?;

    assert_eq!(1, model.files.len());
    assert_eq!(1, model.elements.len());
    Ok(())
}

#[test]
fn given_scip_reference_inside_function_when_projecting_then_reference_and_call_edge_are_preserved()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(&path, sample_index_with_call_edge().write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    assert_eq!(1, model.references.len());
    assert_eq!(
        PRIVATE_SYMBOL,
        model.references[0].referenced_symbol_id.as_str()
    );
    assert_eq!(0, model.references[0].symbol_roles);
    assert_eq!(1, model.call_edges.len());
    assert_eq!(SYMBOL, model.call_edges[0].enclosing_symbol_id.as_str());
    assert_eq!(
        PRIVATE_SYMBOL,
        model.call_edges[0].referenced_symbol_id.as_str()
    );
    Ok(())
}

#[test]
fn given_typed_enclosing_range_when_projecting_then_call_edge_is_preserved()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    let mut index = sample_index_with_call_edge();
    if let Some(document) = index.documents.first_mut() {
        if let Some(definition) = document.occurrences.first_mut() {
            definition.enclosing_range.clear();
            definition.set_multi_line_enclosing_range(MultiLineRange {
                start_line: 0,
                start_character: 0,
                end_line: 3,
                end_character: 1,
                ..Default::default()
            });
        }
        if let Some(reference) = document.occurrences.get_mut(1) {
            reference.range.clear();
            reference.set_single_line_range(SingleLineRange {
                line: 1,
                start_character: 4,
                end_character: 18,
                ..Default::default()
            });
        }
    }
    std::fs::write(&path, index.write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    assert_eq!(1, model.call_edges.len());
    assert_eq!(SYMBOL, model.call_edges[0].enclosing_symbol_id.as_str());
    assert_eq!(
        PRIVATE_SYMBOL,
        model.call_edges[0].referenced_symbol_id.as_str()
    );
    Ok(())
}

#[test]
fn given_legacy_rust_analyzer_signature_when_projecting_then_signature_text_is_recovered()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    let index = sample_index_with_legacy_document_signature_text()?;
    std::fs::write(&path, index.write_to_bytes()?)?;

    let model = project_scip_index(&path, None)?;

    let signature_text = model
        .elements
        .first()
        .and_then(|element| element.signature.as_ref())
        .and_then(|signature| signature.signature_text.as_deref());
    assert_eq!(
        Some("fn public_sum(left: i32, right: i32) -> i32"),
        signature_text
    );
    Ok(())
}

#[test]
fn given_signature_occurrence_when_projecting_then_parameter_and_return_type_scip_ids_are_preserved()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.scip");
    std::fs::write(
        &path,
        sample_index_with_signature_occurrence().write_to_bytes()?,
    )?;

    let model = project_scip_index(&path, None)?;

    let first_parameter = model
        .elements
        .first()
        .and_then(|element| element.signature.as_ref())
        .and_then(|signature| signature.parameters.first());
    assert_eq!(
        Some(PARAMETER_SYMBOL),
        first_parameter.and_then(|parameter| parameter.scip_symbol.as_deref())
    );
    assert!(
        first_parameter
            .and_then(|parameter| parameter.source_span.as_ref())
            .is_some()
    );
    let parameter_type = first_parameter.and_then(|parameter| parameter.type_reference.as_ref());
    assert_eq!(
        Some(PARAMETER_TYPE_SYMBOL),
        parameter_type.and_then(|type_reference| type_reference.scip_symbol.as_deref())
    );
    assert_eq!(
        Some("scip_symbol"),
        parameter_type.map(|type_reference| type_reference.confidence.as_str())
    );
    let return_type = model
        .elements
        .first()
        .and_then(|element| element.signature.as_ref())
        .and_then(|signature| signature.return_type.as_ref());
    assert_eq!(
        Some(RETURN_SYMBOL),
        return_type.and_then(|return_type| return_type.scip_symbol.as_deref())
    );
    assert_eq!(
        Some("scip_symbol"),
        return_type.map(|return_type| return_type.confidence.as_str())
    );
    Ok(())
}

#[test]
fn given_unknown_extension_when_format_is_not_explicit_then_typed_error_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let path = directory.path().join("fixture.data");
    std::fs::write(&path, sample_index().write_to_bytes()?)?;

    let error = project_scip_index(&path, None).err();

    assert!(matches!(error, Some(ScipError::UnknownFormat { .. })));
    Ok(())
}

#[tokio::test]
async fn given_missing_rust_analyzer_when_generating_then_typed_error_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let output_path = directory.path().join("missing.scip");
    let missing_path = directory.path().join("missing-rust-analyzer");

    let error = generate_rust_scip(
        directory.path(),
        &output_path,
        Some(&missing_path),
        None,
        false,
        None,
    )
    .await
    .err();

    assert!(matches!(error, Some(ScipError::RustAnalyzerMissing { .. })));
    Ok(())
}

#[tokio::test]
async fn given_non_not_found_launch_failure_when_generating_then_typed_launch_error_is_returned()
-> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        let directory = tempdir()?;
        let output_path = directory.path().join("permission.scip");
        let script_path = directory.path().join("ra-not-executable");
        std::fs::write(&script_path, "#!/bin/sh\nexit 0\n")?;

        let error = generate_rust_scip(
            directory.path(),
            &output_path,
            Some(&script_path),
            None,
            false,
            None,
        )
        .await
        .err();

        assert!(matches!(
            error,
            Some(ScipError::RustAnalyzerLaunchFailed { .. })
        ));
    }
    Ok(())
}

#[tokio::test]
async fn given_nonzero_rust_analyzer_when_generating_then_stderr_is_captured()
-> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        let directory = tempdir()?;
        let script_path = directory.path().join("ra-fails");
        write_executable_script(&script_path, "#!/bin/sh\necho failed >&2\nexit 7\n")?;
        let output_path = directory.path().join("failed.scip");

        let error = generate_rust_scip_retrying_text_file_busy(
            directory.path(),
            &output_path,
            &script_path,
        )
        .await
        .err();

        assert!(matches!(
            error,
            Some(ScipError::RustAnalyzerFailed { stderr, .. }) if stderr.contains("failed")
        ));
    }
    Ok(())
}

#[tokio::test]
async fn given_successful_rust_analyzer_when_writing_stderr_then_diagnostics_are_returned()
-> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        let directory = tempdir()?;
        let script_path = directory.path().join("ra-diagnostics");
        write_executable_script(
            &script_path,
            "#!/bin/sh\necho indexed >&2\nprintf '' > \"$4\"\nexit 0\n",
        )?;
        let output_path = directory.path().join("generated.scip");

        let stderr = generate_rust_scip_retrying_text_file_busy(
            directory.path(),
            &output_path,
            &script_path,
        )
        .await?;

        assert!(stderr.contains("indexed"));
        assert!(output_path.exists());
    }
    Ok(())
}

#[cfg(unix)]
fn write_executable_script(
    path: &std::path::Path,
    contents: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let temporary_path = path.with_extension("tmp");
    {
        let mut file = std::fs::File::create(&temporary_path)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
    }

    let mut permissions = std::fs::metadata(&temporary_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&temporary_path, permissions)?;
    std::fs::rename(temporary_path, path)?;
    Ok(())
}

#[cfg(unix)]
async fn generate_rust_scip_retrying_text_file_busy(
    project_path: &std::path::Path,
    output_path: &std::path::Path,
    script_path: &std::path::Path,
) -> Result<String, ScipError> {
    let mut attempts = 0;
    loop {
        let result = generate_rust_scip(
            project_path,
            output_path,
            Some(script_path),
            None,
            false,
            None,
        )
        .await;

        if attempts < 5 && is_text_file_busy_launch_failure(&result) {
            attempts += 1;
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }

        return result;
    }
}

#[cfg(unix)]
fn is_text_file_busy_launch_failure(result: &Result<String, ScipError>) -> bool {
    matches!(
        result,
        Err(ScipError::RustAnalyzerLaunchFailed { details, .. })
            if details.contains("Text file busy") || details.contains("os error 26")
    )
}

fn sample_index() -> Index {
    Index {
        metadata: MessageField::some(Metadata {
            project_root: "fixture".to_owned(),
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
            occurrences: vec![Occurrence {
                range: vec![0, 0, 10],
                symbol: SYMBOL.to_owned(),
                symbol_roles: SymbolRole::Definition as i32,
                enclosing_range: vec![0, 0, 3, 1],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn sample_index_with_call_edge() -> Index {
    let mut index = sample_index();
    if let Some(document) = index.documents.first_mut() {
        document.symbols.push(SymbolInformation {
            symbol: PRIVATE_SYMBOL.to_owned(),
            kind: symbol_information::Kind::Function.into(),
            display_name: "private_offset".to_owned(),
            ..Default::default()
        });
        document.occurrences.push(Occurrence {
            range: vec![1, 4, 18],
            symbol: PRIVATE_SYMBOL.to_owned(),
            symbol_roles: 0,
            ..Default::default()
        });
        document.occurrences.push(Occurrence {
            range: vec![4, 0, 14],
            symbol: PRIVATE_SYMBOL.to_owned(),
            symbol_roles: SymbolRole::Definition as i32,
            enclosing_range: vec![4, 0, 6, 1],
            ..Default::default()
        });
    }
    index
}

fn sample_index_with_legacy_document_signature_text() -> Result<Index, Box<dyn std::error::Error>> {
    let mut index = sample_index();
    let legacy_document = Document {
        language: "rust".to_owned(),
        text: "fn public_sum(left: i32, right: i32) -> i32".to_owned(),
        ..Default::default()
    };
    let signature = Signature::parse_from_bytes(&legacy_document.write_to_bytes()?)?;
    if let Some(document) = index.documents.first_mut()
        && let Some(symbol) = document.symbols.first_mut()
    {
        symbol.signature_documentation = MessageField::some(signature);
    }
    Ok(index)
}

fn sample_index_with_signature_occurrence() -> Index {
    let mut index = sample_index();
    let mut signature = Signature {
        language: "rust".to_owned(),
        text: "fn public_sum(left: i32, right: i32) -> i32".to_owned(),
        ..Default::default()
    };
    signature.occurrences.push(Occurrence {
        range: vec![0, 20, 23],
        symbol: PARAMETER_TYPE_SYMBOL.to_owned(),
        ..Default::default()
    });
    signature.occurrences.push(Occurrence {
        range: vec![0, 40, 43],
        symbol: RETURN_SYMBOL.to_owned(),
        ..Default::default()
    });
    if let Some(document) = index.documents.first_mut() {
        document.symbols.push(SymbolInformation {
            symbol: PARAMETER_SYMBOL.to_owned(),
            kind: symbol_information::Kind::Parameter.into(),
            display_name: "left".to_owned(),
            ..Default::default()
        });
        document.occurrences.push(Occurrence {
            range: vec![0, 14, 18],
            symbol: PARAMETER_SYMBOL.to_owned(),
            symbol_roles: SymbolRole::Definition as i32,
            ..Default::default()
        });
        if let Some(symbol) = document.symbols.first_mut() {
            symbol.signature_documentation = MessageField::some(signature);
        }
    }
    index
}
