use crate::{ReportError, ReportResult};

use rr_core::{
    ElementBrief, ElementSummary, EvidenceRecord, FiveWSummary, SemanticModel,
    SymbolReferenceSummary,
};

use std::collections::BTreeMap;

const SCIP_SYMBOL_ROLE_TEST: i32 = 0x20;

pub fn build_element_brief(
    model: &SemanticModel,
    symbol_id: &str,
    include_inferred: bool,
    reference_limit: usize,
) -> ReportResult<ElementBrief> {
    let element = model
        .element_by_id(symbol_id)
        .ok_or_else(|| ReportError::element_not_found(symbol_id))?
        .clone();

    let references: Vec<_> = model
        .references
        .iter()
        .filter(|reference| reference_matches_element(reference, &element))
        .take(reference_limit)
        .cloned()
        .collect();
    let all_references: Vec<_> = model
        .references
        .iter()
        .filter(|reference| reference_matches_element(reference, &element))
        .collect();

    let mut reference_file_counts = BTreeMap::new();
    for reference in &all_references {
        let count = reference_file_counts
            .entry(reference.document_path.clone())
            .or_insert(0_usize);
        *count += 1;
    }

    let mut top_reference_files: Vec<_> = reference_file_counts.into_iter().collect();
    top_reference_files
        .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    let child_symbols: Vec<_> = model
        .elements
        .iter()
        .filter(|child| child.enclosing_symbol.as_deref() == Some(element.scip_symbol.as_str()))
        .collect();

    let test_references: Vec<_> = all_references
        .iter()
        .copied()
        .filter(|reference| is_test_reference(reference))
        .take(reference_limit)
        .collect();

    let outgoing_call_edges = model.outgoing_call_edges(element.symbol_id.as_str());
    let incoming_call_edges = model.incoming_call_edges(element.symbol_id.as_str());

    let mut confirmed = vec![
        "element came from SCIP SymbolInformation".to_owned(),
        format!(
            "kind and display name: {:?} {}",
            element.kind, element.display_name
        ),
    ];
    let mut unknown = vec![
        "git recency is not available from SCIP".to_owned(),
        "SCIP occurrence alone does not prove runtime call behavior".to_owned(),
        "intended public API status is unknown unless facts support it".to_owned(),
    ];
    let mut what_summary = vec![format!("{:?} {}", element.kind, element.display_name)];

    if let Some(signature) = &element.signature {
        confirmed.extend(signature.confirmed.iter().cloned());
        unknown.extend(signature.unknown.iter().cloned());
        if let Some(signature_text) = &signature.signature_text {
            confirmed.push(format!("signature: {signature_text}"));
            what_summary.push(format!("signature: {signature_text}"));
        }
        if !signature.parameters.is_empty() {
            let parameter_names = signature
                .parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            confirmed.push(format!("parameters: {parameter_names}"));
            what_summary.push(format!("parameters: {parameter_names}"));

            let parameter_symbols = signature
                .parameters
                .iter()
                .filter_map(|parameter| parameter.scip_symbol.as_deref())
                .collect::<Vec<_>>();
            if !parameter_symbols.is_empty() {
                confirmed.push(format!(
                    "parameter SCIP IDs: {}",
                    parameter_symbols.join(", ")
                ));
                what_summary.push(format!(
                    "parameter SCIP IDs: {}",
                    parameter_symbols.join(", ")
                ));
            }
        }
        if let Some(return_type) = &signature.return_type {
            confirmed.push(format!("return type: {}", return_type.display_text));
            what_summary.push(format!("returns: {}", return_type.display_text));
            if let Some(return_symbol) = &return_type.scip_symbol {
                confirmed.push(format!("return type SCIP ID: {return_symbol}"));
                what_summary.push(format!("return type SCIP ID: {return_symbol}"));
            } else {
                unknown.push(
                    "return type SCIP symbol is unavailable; only return type text is confirmed"
                        .to_owned(),
                );
            }
        }
    }

    if !child_symbols.is_empty() {
        let child_names = child_symbols
            .iter()
            .map(|child| child.display_name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        confirmed.push(format!("child symbols: {child_names}"));
        what_summary.push(format!("child symbols: {child_names}"));
    }

    let mut where_summary = Vec::new();
    if let Some(span) = &element.definition_span {
        let definition = format!(
            "defined at {}:{}:{}",
            span.document_path, span.start_line, span.start_column
        );
        confirmed.push(definition.clone());
        where_summary.push(definition);
    }
    if !top_reference_files.is_empty() && reference_limit > 0 {
        let top_files = top_reference_files
            .iter()
            .take(reference_limit)
            .map(|(path, count)| format!("{path} ({count})"))
            .collect::<Vec<_>>()
            .join(", ");
        confirmed.push(format!("top reference files: {top_files}"));
        where_summary.push(format!("top reference files: {top_files}"));
    }

    let inferred = if include_inferred && all_references.len() > 3 {
        vec!["many SCIP references may indicate API relevance".to_owned()]
    } else {
        Vec::new()
    };

    let mut who = Vec::new();
    if let Some(package) = &element.package {
        confirmed.push(format!("package/crate: {package}"));
        who.push(package.clone());
    }
    if let Some(enclosing_symbol) = &element.enclosing_symbol {
        confirmed.push(format!("enclosing symbol: {enclosing_symbol}"));
        who.push(enclosing_symbol.clone());
    }

    let outgoing_call_summaries = outgoing_call_edges
        .iter()
        .take(reference_limit)
        .map(|edge| {
            format!(
                "SCIP-derived outgoing function_like_reference to {} at {}:{}:{}",
                edge.referenced_symbol_id.as_str(),
                edge.evidence_span.document_path,
                edge.evidence_span.start_line,
                edge.evidence_span.start_column
            )
        })
        .collect::<Vec<_>>();
    let incoming_call_summaries = incoming_call_edges
        .iter()
        .take(reference_limit)
        .map(|edge| {
            format!(
                "SCIP-derived incoming function_like_reference from {} at {}:{}:{}",
                edge.enclosing_symbol_id.as_str(),
                edge.evidence_span.document_path,
                edge.evidence_span.start_line,
                edge.evidence_span.start_column
            )
        })
        .collect::<Vec<_>>();

    confirmed.extend(outgoing_call_summaries.iter().cloned());
    confirmed.extend(incoming_call_summaries.iter().cloned());

    Ok(ElementBrief {
        element: element.clone(),
        five_w: FiveWSummary {
            who,
            what: what_summary,
            where_: where_summary,
            when: vec!["unknown: not available from SCIP".to_owned()],
            why: inferred.clone(),
            how: child_symbols
                .iter()
                .map(|child| format!("child {:?} {}", child.kind, child.display_name))
                .chain(test_references.iter().map(|reference| {
                    format!(
                        "test reference at {}:{}:{}",
                        reference.source_span.document_path,
                        reference.source_span.start_line,
                        reference.source_span.start_column
                    )
                }))
                .chain(outgoing_call_summaries.iter().cloned())
                .chain(incoming_call_summaries.iter().cloned())
                .collect(),
        },
        evidence: references
            .iter()
            .map(|reference| EvidenceRecord {
                id: format!(
                    "evidence:{}:{}:{}",
                    reference.source_span.document_path,
                    reference.source_span.start_line,
                    reference.source_span.start_column
                ),
                kind: "SCIP occurrence".to_owned(),
                confidence_label: "confirmed".to_owned(),
                source_spans: vec![reference.source_span.clone()],
                confirmed: vec!["reference came from a SCIP occurrence".to_owned()],
                inferred: Vec::new(),
                unknown: vec![
                    "SCIP occurrence alone does not prove runtime call behavior".to_owned(),
                ],
            })
            .collect(),
        confirmed,
        inferred,
        unknown,
    })
}

fn reference_matches_element(reference: &SymbolReferenceSummary, element: &ElementSummary) -> bool {
    let referenced_symbol_id = reference.referenced_symbol_id.as_str();
    referenced_symbol_id == element.symbol_id.as_str()
        || referenced_symbol_id == element.stable_id.as_str()
        || referenced_symbol_id == element.scip_symbol.as_str()
}

fn is_test_reference(reference: &SymbolReferenceSummary) -> bool {
    reference.symbol_roles & SCIP_SYMBOL_ROLE_TEST != 0
        || reference.document_path.starts_with("tests/")
        || reference.document_path.contains("/tests/")
        || reference.document_path.ends_with("_test.rs")
        || reference.document_path.ends_with("_tests.rs")
}
