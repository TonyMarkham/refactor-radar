use rr_core::{
    CallEdgeSummary, ElementKind, ElementSummary, FileSummary, FunctionParameterSummary,
    FunctionSignatureSummary, Language, ProjectSummary, SemanticModel, SourceSpan, StableId,
    SymbolId, SymbolReferenceSummary, TypeReferenceSummary,
};
use rr_report::{ReportError, build_element_brief, build_project_summary, generate_llm_brief};

const ENUM_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/Status#";
const READY_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/Status#Ready.";
const FUNCTION_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/parse_record().";
const HELPER_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/private_offset().";
const PARAM_SYMBOL: &str = "local 1";
const RETURN_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/Record#";

#[test]
fn given_project_model_when_building_summary_then_counts_and_hotspots_are_reported()
-> Result<(), Box<dyn std::error::Error>> {
    let model = sample_model()?;

    let summary = build_project_summary(&model);

    assert_eq!("fixture", summary.project_id);
    assert_eq!(1, summary.document_count);
    assert_eq!(4, summary.element_count);
    assert_eq!(1, summary.reference_count);
    assert_eq!(vec!["src/lib.rs (1)"], summary.hotspot_files);
    Ok(())
}

#[test]
fn given_enum_element_when_building_brief_then_variants_and_reference_hotspots_are_reported()
-> Result<(), Box<dyn std::error::Error>> {
    let model = sample_model()?;

    let brief = build_element_brief(&model, ENUM_SYMBOL, true, 10)?;

    assert!(
        brief
            .five_w
            .what
            .iter()
            .any(|entry| entry.contains("Ready"))
    );
    assert!(
        brief
            .five_w
            .where_
            .iter()
            .any(|entry| entry.contains("src/lib.rs (1)"))
    );
    assert!(
        !brief
            .unknown
            .iter()
            .any(|entry| entry.contains("return type SCIP symbol"))
    );
    Ok(())
}

#[test]
fn given_stable_id_reference_when_building_brief_then_lookup_and_reference_matching_work()
-> Result<(), Box<dyn std::error::Error>> {
    let mut model = sample_model()?;
    let stable_id = StableId::from_scip_symbol(FUNCTION_SYMBOL);
    let stable_span = SourceSpan::from_scip_range("fixture", "src/stable.rs", &[3, 4, 15])?;
    model.references.push(SymbolReferenceSummary {
        referenced_symbol_id: SymbolId::new(stable_id.as_str()),
        source_span: stable_span,
        document_path: "src/stable.rs".to_owned(),
        symbol_roles: 0,
    });

    let brief = build_element_brief(&model, stable_id.as_str(), true, 10)?;

    assert!(brief.evidence.iter().any(|evidence| {
        evidence
            .source_spans
            .iter()
            .any(|span| span.document_path == "src/stable.rs")
    }));
    assert!(
        brief
            .five_w
            .where_
            .iter()
            .any(|entry| entry.contains("src/stable.rs (1)"))
    );
    Ok(())
}

#[test]
fn given_missing_element_when_building_brief_then_typed_error_and_message_are_returned()
-> Result<(), Box<dyn std::error::Error>> {
    let model = sample_model()?;

    let result = build_element_brief(&model, "missing-symbol", true, 10);

    assert!(result.is_err());
    let Some(error) = result.err() else {
        return Ok(());
    };
    assert_eq!(
        "element was not found in the projected semantic model",
        error.message()
    );
    match error {
        ReportError::ElementNotFound { symbol_id, .. } => {
            assert_eq!("missing-symbol", symbol_id);
        }
    }
    Ok(())
}

#[test]
fn given_reference_limit_and_disabled_inference_when_building_brief_then_outputs_are_capped()
-> Result<(), Box<dyn std::error::Error>> {
    let mut model = sample_model()?;
    for (path, line) in [
        ("src/a.rs", 10),
        ("src/a.rs", 11),
        ("src/b.rs", 12),
        ("src/c.rs", 13),
    ] {
        let source_span = SourceSpan::from_scip_range("fixture", path, &[line, 0, 1])?;
        model.references.push(SymbolReferenceSummary {
            referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
            source_span,
            document_path: path.to_owned(),
            symbol_roles: 0,
        });
    }

    let brief = build_element_brief(&model, FUNCTION_SYMBOL, false, 1)?;

    assert_eq!(1, brief.evidence.len());
    assert!(brief.inferred.is_empty());
    assert!(brief.five_w.why.is_empty());
    assert!(
        brief
            .five_w
            .where_
            .iter()
            .any(|entry| entry == "top reference files: src/a.rs (2)")
    );
    assert!(
        !brief
            .five_w
            .where_
            .iter()
            .any(|entry| entry.contains("src/b.rs"))
    );
    assert!(
        !brief
            .five_w
            .where_
            .iter()
            .any(|entry| entry.contains("src/c.rs"))
    );
    Ok(())
}

#[test]
fn given_function_element_when_building_brief_then_signature_and_function_like_references_are_reported()
-> Result<(), Box<dyn std::error::Error>> {
    let model = sample_model()?;

    let brief = build_element_brief(&model, FUNCTION_SYMBOL, true, 10)?;
    let helper_brief = build_element_brief(&model, HELPER_SYMBOL, true, 10)?;

    assert!(brief.five_w.what.iter().any(|entry| {
        entry.contains("signature: fn parse_record(input: &str) -> Result<Record, FixtureError>")
    }));
    assert!(
        brief
            .five_w
            .how
            .iter()
            .any(|entry| entry.contains("SCIP-derived outgoing function_like_reference"))
    );
    assert!(
        brief
            .five_w
            .when
            .iter()
            .any(|entry| entry.contains("not available from SCIP"))
    );
    assert!(
        brief
            .confirmed
            .iter()
            .any(|entry| entry.contains("signature text came from SCIP"))
    );
    assert!(
        brief
            .confirmed
            .iter()
            .any(|entry| entry.contains("SCIP-derived outgoing function_like_reference"))
    );
    assert!(
        helper_brief
            .five_w
            .how
            .iter()
            .any(|entry| entry.contains("SCIP-derived incoming function_like_reference"))
    );
    assert!(
        helper_brief
            .confirmed
            .iter()
            .any(|entry| entry.contains("SCIP-derived incoming function_like_reference"))
    );
    assert!(
        brief
            .unknown
            .iter()
            .any(|entry| entry
                .contains("SCIP occurrence alone does not prove runtime call behavior"))
    );
    assert!(
        brief
            .unknown
            .iter()
            .any(|entry| entry.contains("intended public API status is unknown"))
    );
    assert!(
        brief
            .unknown
            .iter()
            .any(|entry| entry.contains("return type SCIP symbol is unavailable"))
    );
    assert!(
        brief
            .unknown
            .iter()
            .any(|entry| entry.contains("git recency"))
    );
    Ok(())
}

#[test]
fn given_test_reference_when_building_brief_then_how_labels_test_reference()
-> Result<(), Box<dyn std::error::Error>> {
    let mut model = sample_model()?;
    let test_span = SourceSpan::from_scip_range("fixture", "tests/parse_record.rs", &[1, 2, 14])?;
    model.references.push(SymbolReferenceSummary {
        referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
        source_span: test_span,
        document_path: "tests/parse_record.rs".to_owned(),
        symbol_roles: 0,
    });
    let role_span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[2, 3, 9])?;
    model.references.push(SymbolReferenceSummary {
        referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
        source_span: role_span,
        document_path: "src/lib.rs".to_owned(),
        symbol_roles: 0x20,
    });
    let singular_suffix_span =
        SourceSpan::from_scip_range("fixture", "src/parse_record_test.rs", &[3, 4, 10])?;
    model.references.push(SymbolReferenceSummary {
        referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
        source_span: singular_suffix_span,
        document_path: "src/parse_record_test.rs".to_owned(),
        symbol_roles: 0,
    });
    let plural_suffix_span =
        SourceSpan::from_scip_range("fixture", "src/parse_record_tests.rs", &[4, 5, 11])?;
    model.references.push(SymbolReferenceSummary {
        referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
        source_span: plural_suffix_span,
        document_path: "src/parse_record_tests.rs".to_owned(),
        symbol_roles: 0,
    });

    let brief = build_element_brief(&model, FUNCTION_SYMBOL, true, 10)?;

    assert!(
        brief
            .five_w
            .how
            .iter()
            .any(|entry| entry.contains("test reference at tests/parse_record.rs"))
    );
    assert!(
        brief
            .five_w
            .how
            .iter()
            .any(|entry| entry.contains("test reference at src/lib.rs"))
    );
    assert!(
        brief
            .five_w
            .how
            .iter()
            .any(|entry| entry.contains("test reference at src/parse_record_test.rs"))
    );
    assert!(
        brief
            .five_w
            .how
            .iter()
            .any(|entry| entry.contains("test reference at src/parse_record_tests.rs"))
    );
    Ok(())
}

#[test]
fn given_function_signature_with_scip_ids_when_building_brief_then_ids_are_reported()
-> Result<(), Box<dyn std::error::Error>> {
    let mut model = sample_model()?;
    if let Some(element) = model
        .elements
        .iter_mut()
        .find(|element| element.scip_symbol == FUNCTION_SYMBOL)
    {
        element.signature = Some(FunctionSignatureSummary {
            signature_text: Some("fn parse_record(input: &str) -> Record".to_owned()),
            parameters: vec![FunctionParameterSummary {
                name: "input".to_owned(),
                scip_symbol: Some(PARAM_SYMBOL.to_owned()),
                parameter_kind: "Parameter".to_owned(),
                source_span: None,
                type_reference: Some(TypeReferenceSummary {
                    display_text: "&str".to_owned(),
                    scip_symbol: None,
                    source_span: None,
                    signature_range: None,
                    confidence: "signature_text_only".to_owned(),
                }),
            }],
            return_type: Some(TypeReferenceSummary {
                display_text: "Record".to_owned(),
                scip_symbol: Some(RETURN_SYMBOL.to_owned()),
                source_span: None,
                signature_range: None,
                confidence: "scip_symbol".to_owned(),
            }),
            confirmed: vec!["signature text came from SCIP signature_documentation".to_owned()],
            unknown: Vec::new(),
        });
    }

    let brief = build_element_brief(&model, FUNCTION_SYMBOL, true, 10)?;

    assert!(
        brief
            .five_w
            .what
            .iter()
            .any(|entry| entry.contains(PARAM_SYMBOL))
    );
    assert!(
        brief
            .five_w
            .what
            .iter()
            .any(|entry| entry.contains(RETURN_SYMBOL))
    );
    assert!(
        !brief
            .unknown
            .iter()
            .any(|entry| entry.contains("return type SCIP symbol is unavailable"))
    );
    Ok(())
}

#[test]
fn given_llm_brief_when_generated_then_confirmed_inferred_and_unknown_sections_are_rendered()
-> Result<(), Box<dyn std::error::Error>> {
    let model = sample_model()?;

    let brief = generate_llm_brief(&model, None)?;

    assert!(brief.contains("Confirmed:"));
    assert!(brief.contains("Inferred:"));
    assert!(brief.contains("Unknown:"));
    Ok(())
}

#[test]
fn given_token_budget_when_generating_llm_brief_then_output_stays_within_budget()
-> Result<(), Box<dyn std::error::Error>> {
    let mut model = sample_model()?;
    for column in [1, 2, 3, 4] {
        let source_span =
            SourceSpan::from_scip_range("fixture", "src/lib.rs", &[9, column, column + 1])?;
        model.references.push(SymbolReferenceSummary {
            referenced_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
            source_span,
            document_path: "src/lib.rs".to_owned(),
            symbol_roles: 0,
        });
    }

    let brief = generate_llm_brief(&model, Some(250))?;

    assert!(brief.len() <= 1000);
    assert!(brief.contains("# Project fixture"));
    assert!(brief.contains("## parse_record"));
    assert!(!brief.contains("## Status"));
    assert!(!brief.contains("## Ready"));
    Ok(())
}

fn sample_model() -> Result<SemanticModel, Box<dyn std::error::Error>> {
    let enum_span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[0, 0, 6])?;
    let function_span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[4, 0, 12])?;
    let reference_span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[8, 10, 16])?;

    Ok(SemanticModel {
        project: ProjectSummary {
            project_id: "fixture".to_owned(),
            project_root: "fixture".to_owned(),
            producer_name: Some("rust-analyzer".to_owned()),
            producer_version: Some("test".to_owned()),
        },
        files: vec![FileSummary {
            document_path: "src/lib.rs".to_owned(),
            language: "rust".to_owned(),
            symbol_count: 4,
            occurrence_count: 4,
        }],
        elements: vec![
            ElementSummary {
                stable_id: StableId::from_scip_symbol(ENUM_SYMBOL),
                symbol_id: SymbolId::new(ENUM_SYMBOL),
                scip_symbol: ENUM_SYMBOL.to_owned(),
                language: Language::Rust,
                kind: ElementKind::Enum,
                display_name: "Status".to_owned(),
                package: Some("basic_crate".to_owned()),
                enclosing_symbol: None,
                definition_span: Some(enum_span.clone()),
                signature: None,
                documentation: Vec::new(),
                reference_count: 1,
            },
            ElementSummary {
                stable_id: StableId::from_scip_symbol(READY_SYMBOL),
                symbol_id: SymbolId::new(READY_SYMBOL),
                scip_symbol: READY_SYMBOL.to_owned(),
                language: Language::Rust,
                kind: ElementKind::EnumMember,
                display_name: "Ready".to_owned(),
                package: Some("basic_crate".to_owned()),
                enclosing_symbol: Some(ENUM_SYMBOL.to_owned()),
                definition_span: None,
                signature: None,
                documentation: Vec::new(),
                reference_count: 0,
            },
            ElementSummary {
                stable_id: StableId::from_scip_symbol(FUNCTION_SYMBOL),
                symbol_id: SymbolId::new(FUNCTION_SYMBOL),
                scip_symbol: FUNCTION_SYMBOL.to_owned(),
                language: Language::Rust,
                kind: ElementKind::Function,
                display_name: "parse_record".to_owned(),
                package: Some("basic_crate".to_owned()),
                enclosing_symbol: None,
                definition_span: Some(function_span.clone()),
                signature: Some(FunctionSignatureSummary {
                    signature_text: Some(
                        "fn parse_record(input: &str) -> Result<Record, FixtureError>".to_owned(),
                    ),
                    parameters: Vec::new(),
                    return_type: Some(TypeReferenceSummary {
                        display_text: "Result<Record, FixtureError>".to_owned(),
                        scip_symbol: None,
                        source_span: None,
                        signature_range: None,
                        confidence: "signature_text_only".to_owned(),
                    }),
                    confirmed: vec![
                        "signature text came from SCIP signature_documentation".to_owned(),
                    ],
                    unknown: vec![
                        "return type SCIP symbol is unavailable when SCIP emitted only text"
                            .to_owned(),
                    ],
                }),
                documentation: Vec::new(),
                reference_count: 4,
            },
            ElementSummary {
                stable_id: StableId::from_scip_symbol(HELPER_SYMBOL),
                symbol_id: SymbolId::new(HELPER_SYMBOL),
                scip_symbol: HELPER_SYMBOL.to_owned(),
                language: Language::Rust,
                kind: ElementKind::Function,
                display_name: "private_offset".to_owned(),
                package: Some("basic_crate".to_owned()),
                enclosing_symbol: None,
                definition_span: None,
                signature: None,
                documentation: Vec::new(),
                reference_count: 1,
            },
        ],
        references: vec![SymbolReferenceSummary {
            referenced_symbol_id: SymbolId::new(ENUM_SYMBOL),
            source_span: reference_span.clone(),
            document_path: "src/lib.rs".to_owned(),
            symbol_roles: 0,
        }],
        call_edges: vec![CallEdgeSummary {
            enclosing_symbol_id: SymbolId::new(FUNCTION_SYMBOL),
            referenced_symbol_id: SymbolId::new(HELPER_SYMBOL),
            evidence_span: reference_span,
            confidence: "scip_reference_inside_function_enclosing_range".to_owned(),
        }],
        spans: vec![enum_span, function_span],
    })
}
