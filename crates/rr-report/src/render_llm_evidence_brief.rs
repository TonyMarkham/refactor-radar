use crate::{ReportResult, build_element_brief, build_project_summary};

use rr_core::{ElementSummary, SemanticModel, SymbolReferenceSummary};

pub fn render_llm_evidence_brief(
    model: &SemanticModel,
    budget_tokens: Option<usize>,
) -> ReportResult<String> {
    let project_summary = build_project_summary(model);
    let max_chars = budget_tokens.map(|tokens| tokens.saturating_mul(4));
    let mut output = format!(
        "# Project {}\n\nDocuments: {}\nElements: {}\nReferences: {}\nHotspots: {}\n",
        project_summary.project_id,
        project_summary.document_count,
        project_summary.element_count,
        project_summary.reference_count,
        project_summary.hotspot_files.join(", ")
    );

    let mut ranked_elements: Vec<_> = model
        .elements
        .iter()
        .map(|element| (element, matched_reference_count(model, element)))
        .collect();
    ranked_elements.sort_by(|(left, left_count), (right, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.symbol_id.as_str().cmp(right.symbol_id.as_str()))
    });

    for (element, _) in ranked_elements.into_iter().take(25) {
        let brief = build_element_brief(model, element.symbol_id.as_str(), true, 5)?;
        let section = format!(
            "\n## {}\nConfirmed: {}\nInferred: {}\nUnknown: {}\n",
            brief.element.display_name,
            brief.confirmed.join("; "),
            brief.inferred.join("; "),
            brief.unknown.join("; ")
        );
        if !append_budgeted_section(&mut output, &section, max_chars) {
            break;
        }
    }

    Ok(output)
}

fn matched_reference_count(model: &SemanticModel, element: &ElementSummary) -> usize {
    model
        .references
        .iter()
        .filter(|reference| reference_matches_element(reference, element))
        .count()
}

fn reference_matches_element(reference: &SymbolReferenceSummary, element: &ElementSummary) -> bool {
    let referenced_symbol_id = reference.referenced_symbol_id.as_str();
    referenced_symbol_id == element.symbol_id.as_str()
        || referenced_symbol_id == element.stable_id.as_str()
        || referenced_symbol_id == element.scip_symbol.as_str()
}

fn append_budgeted_section(output: &mut String, section: &str, max_chars: Option<usize>) -> bool {
    if let Some(max_chars) = max_chars
        && output.len().saturating_add(section.len()) > max_chars
    {
        return false;
    }
    output.push_str(section);
    true
}
