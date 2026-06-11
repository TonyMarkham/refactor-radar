use rr_core::{
    CallEdgeSummary, CoreError, ElementBrief, ElementKind, ElementSummary, EvidenceRecord,
    FileSummary, FiveWSummary, Language, ProjectSummary, SemanticModel, SourceSpan, StableId,
    SymbolId, SymbolReferenceSummary,
};

use serde_json::Value;

const SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().";
const CALLEE_SYMBOL: &str = "rust-analyzer cargo basic_crate 0.1.0 basic_crate/internal_sum().";

#[test]
fn given_scip_symbol_when_creating_stable_id_then_generation_is_deterministic() {
    let first = StableId::from_scip_symbol(SYMBOL);
    let second = StableId::from_scip_symbol(SYMBOL);

    assert_eq!(first, second);
    assert_eq!(
        "scip:rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().",
        first.as_str()
    );
}

#[test]
fn given_element_summary_when_created_then_original_scip_symbol_is_preserved() {
    let element = sample_element();

    assert_eq!(SYMBOL, element.scip_symbol.as_str());
    assert_eq!(SYMBOL, element.symbol_id.as_str());
}

#[test]
fn given_zero_based_scip_range_when_projecting_span_then_public_fields_are_one_based()
-> Result<(), Box<dyn std::error::Error>> {
    let span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[0, 2, 5])?;

    assert_eq!("fixture", span.project_id);
    assert_eq!("fixture::src/lib.rs:1:3", span.id.as_str());
    assert_eq!("src/lib.rs", span.document_path);
    assert_eq!(1, span.start_line);
    assert_eq!(3, span.start_column);
    assert_eq!(1, span.end_line);
    assert_eq!(6, span.end_column);

    let multi_line_span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[1, 3, 2, 7])?;

    assert_eq!("fixture::src/lib.rs:2:4", multi_line_span.id.as_str());
    assert_eq!(2, multi_line_span.start_line);
    assert_eq!(4, multi_line_span.start_column);
    assert_eq!(3, multi_line_span.end_line);
    assert_eq!(8, multi_line_span.end_column);
    Ok(())
}

#[test]
fn given_malformed_scip_ranges_when_projecting_span_then_invalid_ranges_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let zero_width = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[2, 4, 4])?;

    assert_eq!(3, zero_width.start_line);
    assert_eq!(5, zero_width.start_column);
    assert_eq!(3, zero_width.end_line);
    assert_eq!(5, zero_width.end_column);

    let invalid_ranges: &[&[i32]] = &[&[], &[0, 1], &[-1, 0, 1], &[1, 3, 0, 4], &[1, 4, 1, 3]];

    for range in invalid_ranges {
        let result = SourceSpan::from_scip_range("fixture", "src/lib.rs", range);

        assert!(matches!(&result, Err(CoreError::InvalidScipRange { .. })));
        if let Err(error) = result {
            assert_eq!(
                "SCIP range must contain valid zero-based positions",
                error.message()
            );
        }
    }

    Ok(())
}

#[test]
fn given_element_brief_when_serializing_then_five_w_and_evidence_labels_are_preserved()
-> Result<(), Box<dyn std::error::Error>> {
    let brief = ElementBrief {
        element: sample_element(),
        five_w: FiveWSummary {
            who: vec!["basic_crate".to_owned()],
            what: vec!["Function public_sum".to_owned()],
            where_: vec!["defined at src/lib.rs:1:1".to_owned()],
            when: vec!["not available from SCIP".to_owned()],
            why: vec!["many SCIP references may indicate API relevance".to_owned()],
            how: vec!["SCIP-derived outgoing function_like_reference".to_owned()],
        },
        evidence: vec![EvidenceRecord {
            id: "evidence:src/lib.rs:1:1".to_owned(),
            kind: "SCIP occurrence".to_owned(),
            confidence_label: "confirmed".to_owned(),
            source_spans: Vec::new(),
            confirmed: vec!["reference came from a SCIP occurrence".to_owned()],
            inferred: Vec::new(),
            unknown: vec!["SCIP occurrence alone does not prove runtime call behavior".to_owned()],
        }],
        confirmed: vec!["element came from SCIP SymbolInformation".to_owned()],
        inferred: vec!["many SCIP references may indicate API relevance".to_owned()],
        unknown: vec!["git recency is not available from SCIP".to_owned()],
    };

    let value = serde_json::to_value(brief)?;
    assert!(
        value
            .get("five_w")
            .and_then(|five_w| five_w.get("who"))
            .is_some()
    );
    assert!(
        value
            .get("five_w")
            .and_then(|five_w| five_w.get("what"))
            .is_some()
    );
    assert!(
        value
            .get("five_w")
            .and_then(|five_w| five_w.get("where"))
            .is_some()
    );
    assert!(
        value
            .get("five_w")
            .and_then(|five_w| five_w.get("when"))
            .is_some()
    );
    assert!(
        value
            .get("five_w")
            .and_then(|five_w| five_w.get("why"))
            .is_some()
    );
    assert!(
        value
            .get("five_w")
            .and_then(|five_w| five_w.get("how"))
            .is_some()
    );
    assert!(matches!(value.get("evidence"), Some(Value::Array(_))));
    assert!(matches!(value.get("confirmed"), Some(Value::Array(_))));
    assert!(matches!(value.get("inferred"), Some(Value::Array(_))));
    assert!(matches!(value.get("unknown"), Some(Value::Array(_))));

    let evidence = value
        .get("evidence")
        .and_then(Value::as_array)
        .and_then(|records| records.first());
    assert!(
        evidence
            .and_then(|record| record.get("confirmed"))
            .is_some()
    );
    assert!(evidence.and_then(|record| record.get("inferred")).is_some());
    assert!(evidence.and_then(|record| record.get("unknown")).is_some());
    Ok(())
}

#[test]
fn given_semantic_model_when_serializing_then_ids_are_strings_and_helpers_resolve()
-> Result<(), Box<dyn std::error::Error>> {
    let span = SourceSpan::from_scip_range("fixture", "src/lib.rs", &[0, 0, 10])?;
    let model = SemanticModel {
        project: ProjectSummary {
            project_id: "fixture".to_owned(),
            project_root: "/tmp/fixture".to_owned(),
            producer_name: Some("test-producer".to_owned()),
            producer_version: Some("0.1.0".to_owned()),
        },
        files: vec![FileSummary {
            document_path: "src/lib.rs".to_owned(),
            language: "Rust".to_owned(),
            symbol_count: 1,
            occurrence_count: 1,
        }],
        elements: vec![sample_element()],
        references: vec![SymbolReferenceSummary {
            referenced_symbol_id: SymbolId::new(SYMBOL),
            source_span: span.clone(),
            document_path: "src/lib.rs".to_owned(),
            symbol_roles: 1 | 8,
        }],
        call_edges: vec![CallEdgeSummary {
            enclosing_symbol_id: SymbolId::new(SYMBOL),
            referenced_symbol_id: SymbolId::new(CALLEE_SYMBOL),
            evidence_span: span.clone(),
            confidence: "inferred".to_owned(),
        }],
        spans: vec![span.clone()],
    };

    assert!(model.has_project_id("fixture"));
    assert!(
        model
            .element_by_id("scip:rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().")
            .is_some()
    );
    assert!(model.element_by_id(SYMBOL).is_some());
    assert!(model.span_by_id(span.id.as_str()).is_some());
    assert_eq!(1, model.outgoing_call_edges(SYMBOL).len());
    assert_eq!(1, model.incoming_call_edges(CALLEE_SYMBOL).len());

    let value = serde_json::to_value(&model)?;
    let element = value
        .get("elements")
        .and_then(Value::as_array)
        .and_then(|elements| elements.first());
    assert_eq!(
        Some(Value::String(SYMBOL.to_owned())),
        element
            .and_then(|element| element.get("symbol_id"))
            .cloned()
    );
    assert_eq!(
        Some(Value::String(
            "scip:rust-analyzer cargo basic_crate 0.1.0 basic_crate/public_sum().".to_owned()
        )),
        element
            .and_then(|element| element.get("stable_id"))
            .cloned()
    );

    let serialized = serde_json::to_string(&model)?;
    let round_trip: SemanticModel = serde_json::from_str(&serialized)?;

    assert_eq!(model, round_trip);
    Ok(())
}

fn sample_element() -> ElementSummary {
    ElementSummary {
        stable_id: StableId::from_scip_symbol(SYMBOL),
        symbol_id: SymbolId::new(SYMBOL),
        scip_symbol: SYMBOL.to_owned(),
        language: Language::Rust,
        kind: ElementKind::Function,
        display_name: "public_sum".to_owned(),
        package: Some("basic_crate".to_owned()),
        enclosing_symbol: None,
        definition_span: None,
        signature: None,
        documentation: Vec::new(),
        reference_count: 1,
    }
}
