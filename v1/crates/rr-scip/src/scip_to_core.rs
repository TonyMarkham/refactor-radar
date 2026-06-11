use crate::{ScipError, ScipResult, project_symbol_kind};
use rr_core::{
    CallEdgeSummary, ElementKind, ElementSummary, FileSummary, FunctionParameterSummary,
    FunctionSignatureSummary, Language, ProjectSummary, SemanticModel, SourceSpan, StableId,
    SymbolId, SymbolReferenceSummary, TypeReferenceSummary,
};

use scip::{
    symbol::parse_symbol,
    types::{Document, Index, Occurrence, Signature, SymbolInformation, SymbolRole, occurrence},
};

pub fn scip_to_core(index: &Index) -> ScipResult<SemanticModel> {
    let metadata = index.metadata.as_ref();
    let project_root = metadata
        .map(|metadata| metadata.project_root.clone())
        .unwrap_or_default();
    let project_id = project_root.clone();
    let tool_info = metadata.and_then(|metadata| metadata.tool_info.as_ref());
    let mut files = Vec::new();
    let mut elements = Vec::new();
    let mut references = Vec::new();
    let mut call_edges = Vec::new();
    let mut spans = Vec::new();

    for document in &index.documents {
        files.push(FileSummary {
            document_path: document.relative_path.clone(),
            language: document.language.clone(),
            symbol_count: document.symbols.len(),
            occurrence_count: document.occurrences.len(),
        });

        for symbol in &document.symbols {
            let definition_span = document
                .occurrences
                .iter()
                .find(|occurrence| {
                    occurrence.symbol == symbol.symbol
                        && occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
                })
                .map(|occurrence| {
                    let range = occurrence_range(occurrence)
                        .ok_or_else(ScipError::core_projection_failed)?;
                    SourceSpan::from_scip_range(&project_id, &document.relative_path, &range)
                        .map_err(|_| ScipError::core_projection_failed())
                })
                .transpose()?;
            if let Some(span) = &definition_span {
                spans.push(span.clone());
            }
            let reference_count = reference_count(index, &symbol.symbol);
            let signature = symbol
                .signature_documentation
                .as_ref()
                .map(|signature| {
                    project_signature(
                        signature,
                        &symbol.symbol,
                        &document.symbols,
                        &document.occurrences,
                        &project_id,
                        &document.relative_path,
                    )
                })
                .transpose()?;
            elements.push(ElementSummary {
                stable_id: StableId::from_scip_symbol(&symbol.symbol),
                symbol_id: SymbolId::new(symbol.symbol.clone()),
                scip_symbol: symbol.symbol.clone(),
                language: Language::Rust,
                kind: symbol
                    .kind
                    .enum_value()
                    .ok()
                    .map(project_symbol_kind)
                    .unwrap_or_else(|| ElementKind::Unknown("scip::UnspecifiedKind".to_owned())),
                display_name: symbol.display_name.clone(),
                package: package_name(&symbol.symbol),
                enclosing_symbol: (!symbol.enclosing_symbol.is_empty())
                    .then(|| symbol.enclosing_symbol.clone()),
                definition_span,
                signature,
                documentation: symbol.documentation.clone(),
                reference_count,
            });
        }

        for occurrence in &document.occurrences {
            if occurrence.symbol.is_empty()
                || occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
            {
                continue;
            }
            let range =
                occurrence_range(occurrence).ok_or_else(ScipError::core_projection_failed)?;
            let source_span =
                SourceSpan::from_scip_range(&project_id, &document.relative_path, &range)
                    .map_err(|_| ScipError::core_projection_failed())?;
            spans.push(source_span.clone());
            references.push(SymbolReferenceSummary {
                referenced_symbol_id: SymbolId::new(occurrence.symbol.clone()),
                source_span,
                document_path: document.relative_path.clone(),
                symbol_roles: occurrence.symbol_roles,
            });
        }
        call_edges.extend(project_call_edges(index, document, &project_id)?);
    }

    Ok(SemanticModel {
        project: ProjectSummary {
            project_id: project_id.clone(),
            project_root,
            producer_name: tool_info.map(|tool_info| tool_info.name.clone()),
            producer_version: tool_info.map(|tool_info| tool_info.version.clone()),
        },
        files,
        elements,
        references,
        call_edges,
        spans,
    })
}

fn project_call_edges(
    index: &Index,
    document: &Document,
    project_id: &str,
) -> ScipResult<Vec<CallEdgeSummary>> {
    let mut call_edges = Vec::new();
    for symbol in document
        .symbols
        .iter()
        .filter(|symbol| is_function_like_symbol(symbol))
    {
        let Some(definition) = document.occurrences.iter().find(|occurrence| {
            occurrence.symbol == symbol.symbol
                && occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
                && occurrence_enclosing_range(occurrence).is_some()
        }) else {
            continue;
        };
        let Some(definition_range) = occurrence_enclosing_range(definition) else {
            continue;
        };
        for occurrence in &document.occurrences {
            let Some(reference_range) = occurrence_range(occurrence) else {
                continue;
            };
            if occurrence.symbol.is_empty()
                || occurrence.symbol == symbol.symbol
                || occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
                || !range_contains(&definition_range, &reference_range)
                || !is_function_like_symbol_id(index, &occurrence.symbol)
            {
                continue;
            }
            let evidence_span =
                SourceSpan::from_scip_range(project_id, &document.relative_path, &reference_range)
                    .map_err(|_| ScipError::core_projection_failed())?;
            call_edges.push(CallEdgeSummary {
                enclosing_symbol_id: SymbolId::new(symbol.symbol.clone()),
                referenced_symbol_id: SymbolId::new(occurrence.symbol.clone()),
                evidence_span,
                confidence: "scip_reference_inside_function_enclosing_range".to_owned(),
            });
        }
    }
    Ok(call_edges)
}

fn is_function_like_symbol_id(index: &Index, symbol_id: &str) -> bool {
    index
        .documents
        .iter()
        .flat_map(|document| document.symbols.iter())
        .chain(index.external_symbols.iter())
        .find(|symbol| symbol.symbol == symbol_id)
        .map(is_function_like_symbol)
        .unwrap_or(false)
}

fn is_function_like_symbol(symbol: &SymbolInformation) -> bool {
    symbol
        .kind
        .enum_value()
        .ok()
        .map(project_symbol_kind)
        .map(|kind| is_function_like_kind(&kind))
        .unwrap_or(false)
}

fn is_function_like_kind(kind: &ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::Function
            | ElementKind::Method
            | ElementKind::StaticMethod
            | ElementKind::TraitMethod
    )
}

fn range_contains(outer: &[i32], inner: &[i32]) -> bool {
    let Some((outer_start_line, outer_start_column, outer_end_line, outer_end_column)) =
        range_bounds(outer)
    else {
        return false;
    };
    let Some((inner_start_line, inner_start_column, inner_end_line, inner_end_column)) =
        range_bounds(inner)
    else {
        return false;
    };
    position_lte(
        (outer_start_line, outer_start_column),
        (inner_start_line, inner_start_column),
    ) && position_lte(
        (inner_end_line, inner_end_column),
        (outer_end_line, outer_end_column),
    )
}

fn range_bounds(range: &[i32]) -> Option<(i32, i32, i32, i32)> {
    let bounds = match range {
        [line, start, end] => (*line, *start, *line, *end),
        [start_line, start_column, end_line, end_column] => {
            (*start_line, *start_column, *end_line, *end_column)
        }
        _ => return None,
    };
    let (start_line, start_column, end_line, end_column) = bounds;
    if start_line < 0
        || start_column < 0
        || end_column < 0
        || end_line < start_line
        || (end_line == start_line && end_column < start_column)
    {
        return None;
    }
    Some(bounds)
}

fn position_lte(left: (i32, i32), right: (i32, i32)) -> bool {
    left.0 < right.0 || (left.0 == right.0 && left.1 <= right.1)
}

fn occurrence_range(occurrence: &Occurrence) -> Option<Vec<i32>> {
    match &occurrence.typed_range {
        Some(occurrence::Typed_range::SingleLineRange(range)) => {
            Some(vec![range.line, range.start_character, range.end_character])
        }
        Some(occurrence::Typed_range::MultiLineRange(range)) => Some(vec![
            range.start_line,
            range.start_character,
            range.end_line,
            range.end_character,
        ]),
        _ if !occurrence.range.is_empty() => Some(occurrence.range.clone()),
        _ => None,
    }
}

fn occurrence_enclosing_range(occurrence: &Occurrence) -> Option<Vec<i32>> {
    match &occurrence.typed_enclosing_range {
        Some(occurrence::Typed_enclosing_range::SingleLineEnclosingRange(range)) => {
            Some(vec![range.line, range.start_character, range.end_character])
        }
        Some(occurrence::Typed_enclosing_range::MultiLineEnclosingRange(range)) => Some(vec![
            range.start_line,
            range.start_character,
            range.end_line,
            range.end_character,
        ]),
        _ if !occurrence.enclosing_range.is_empty() => Some(occurrence.enclosing_range.clone()),
        _ => None,
    }
}

fn reference_count(index: &Index, symbol_id: &str) -> usize {
    index
        .documents
        .iter()
        .flat_map(|document| document.occurrences.iter())
        .filter(|occurrence| {
            occurrence.symbol == symbol_id
                && occurrence.symbol_roles & SymbolRole::Definition as i32 == 0
        })
        .count()
}

fn package_name(symbol_id: &str) -> Option<String> {
    parse_symbol(symbol_id)
        .ok()
        .and_then(|symbol| symbol.package.as_ref().map(|package| package.name.clone()))
        .filter(|name| !name.is_empty())
}

fn project_signature(
    signature: &Signature,
    enclosing_symbol: &str,
    symbols: &[SymbolInformation],
    occurrences: &[Occurrence],
    project_id: &str,
    document_path: &str,
) -> ScipResult<FunctionSignatureSummary> {
    let signature_text = signature_text_from_scip(signature);
    let parameters = parameter_text_ranges(&signature_text)
        .into_iter()
        .map(
            |(parameter_start, parameter_end)| -> ScipResult<FunctionParameterSummary> {
                let parameter_text = &signature_text[parameter_start..parameter_end];
                let display_name = parameter_display_name(parameter_text);
                let parameter_symbol =
                    parameter_symbol_for(enclosing_symbol, &display_name, symbols, occurrences);
                let source_span = parameter_symbol
                    .map(|symbol| {
                        parameter_definition_span(
                            project_id,
                            document_path,
                            &symbol.symbol,
                            occurrences,
                        )
                    })
                    .transpose()?
                    .flatten();
                let parameter_kind = parameter_symbol
                    .and_then(|symbol| symbol.kind.enum_value().ok())
                    .map(project_symbol_kind)
                    .unwrap_or_else(|| {
                        if is_self_parameter(&display_name) {
                            ElementKind::SelfParameter
                        } else {
                            ElementKind::Parameter
                        }
                    });
                let type_reference =
                    parameter_type_range(&signature_text, parameter_start, parameter_end).map(
                        |(type_start, type_end)| {
                            type_reference_from_text(
                                &signature_text[type_start..type_end],
                                type_start,
                                type_end,
                                signature,
                            )
                        },
                    );
                Ok(FunctionParameterSummary {
                    parameter_kind: format!("{parameter_kind:?}"),
                    name: display_name,
                    scip_symbol: parameter_symbol.map(|symbol| symbol.symbol.clone()),
                    source_span,
                    type_reference,
                })
            },
        )
        .collect::<ScipResult<Vec<_>>>()?;
    let return_type = return_type_range(&signature_text).map(|(type_start, type_end)| {
        type_reference_from_text(
            &signature_text[type_start..type_end],
            type_start,
            type_end,
            signature,
        )
    });
    let mut unknown = Vec::new();
    if signature.occurrences.is_empty() {
        unknown.push(
            "SCIP signature occurrences were unavailable; parameter and return types are text-only"
                .to_owned(),
        );
    }
    Ok(FunctionSignatureSummary {
        signature_text: Some(signature_text),
        parameters,
        return_type,
        confirmed: vec!["signature text came from SCIP signature_documentation".to_owned()],
        unknown,
    })
}

fn parameter_definition_span(
    project_id: &str,
    document_path: &str,
    parameter_symbol: &str,
    occurrences: &[Occurrence],
) -> ScipResult<Option<SourceSpan>> {
    occurrences
        .iter()
        .find(|occurrence| {
            occurrence.symbol == parameter_symbol
                && occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
        })
        .map(|occurrence| {
            let range =
                occurrence_range(occurrence).ok_or_else(ScipError::core_projection_failed)?;
            SourceSpan::from_scip_range(project_id, document_path, &range)
                .map_err(|_| ScipError::core_projection_failed())
        })
        .transpose()
}

fn signature_text_from_scip(signature: &Signature) -> String {
    signature.text.clone()
}

fn parameter_display_name(parameter_text: &str) -> String {
    let raw_name = parameter_text
        .split_once(':')
        .map(|(name, _)| name.trim())
        .unwrap_or(parameter_text.trim());
    raw_name
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .trim()
        .to_owned()
}

fn parameter_symbol_for<'a>(
    enclosing_symbol: &str,
    display_name: &str,
    symbols: &'a [SymbolInformation],
    occurrences: &[Occurrence],
) -> Option<&'a SymbolInformation> {
    symbols.iter().find(|symbol| {
        if !is_parameter_symbol(display_name, symbol) {
            return false;
        }
        symbol.enclosing_symbol == enclosing_symbol
            || parameter_definition_inside_enclosing_symbol(
                &symbol.symbol,
                enclosing_symbol,
                occurrences,
            )
    })
}

fn is_parameter_symbol(display_name: &str, symbol: &SymbolInformation) -> bool {
    let Some(kind) = symbol.kind.enum_value().ok().map(project_symbol_kind) else {
        return false;
    };
    match kind {
        ElementKind::SelfParameter => {
            symbol.display_name == display_name || is_self_parameter(display_name)
        }
        ElementKind::Parameter => symbol.display_name == display_name,
        _ => false,
    }
}

fn parameter_definition_inside_enclosing_symbol(
    parameter_symbol: &str,
    enclosing_symbol: &str,
    occurrences: &[Occurrence],
) -> bool {
    let Some(enclosing_range) = occurrences
        .iter()
        .find(|occurrence| {
            occurrence.symbol == enclosing_symbol
                && occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
        })
        .and_then(occurrence_enclosing_range)
    else {
        return false;
    };
    occurrences.iter().any(|occurrence| {
        occurrence.symbol == parameter_symbol
            && occurrence.symbol_roles & SymbolRole::Definition as i32 != 0
            && occurrence_range(occurrence)
                .is_some_and(|range| range_contains(&enclosing_range, &range))
    })
}

fn is_self_parameter(display_name: &str) -> bool {
    display_name == "self"
}

fn parameter_text_ranges(signature_text: &str) -> Vec<(usize, usize)> {
    let Some(open_index) = signature_text.find('(') else {
        return Vec::new();
    };
    let Some(close_index) = matching_close_paren(signature_text, open_index) else {
        return Vec::new();
    };
    split_top_level_commas_with_offsets(signature_text, open_index + 1, close_index)
        .into_iter()
        .filter_map(|(start, end)| trimmed_range(signature_text, start, end))
        .collect()
}

fn parameter_type_range(
    signature_text: &str,
    parameter_start: usize,
    parameter_end: usize,
) -> Option<(usize, usize)> {
    let parameter_text = signature_text.get(parameter_start..parameter_end)?;
    let colon_index = parameter_text.find(':')?;
    trimmed_range(
        signature_text,
        parameter_start + colon_index + ':'.len_utf8(),
        parameter_end,
    )
}

fn return_type_range(signature_text: &str) -> Option<(usize, usize)> {
    let open_index = signature_text.find('(')?;
    let close_index = matching_close_paren(signature_text, open_index)?;
    let after_params = signature_text.get(close_index + 1..)?;
    let arrow_index = after_params.find("->")?;
    trimmed_range(
        signature_text,
        close_index + 1 + arrow_index + "->".len(),
        signature_text.len(),
    )
}

fn trimmed_range(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let slice = text.get(start..end)?;
    let trimmed_start = start + slice.len() - slice.trim_start().len();
    let trimmed_end = end - (slice.len() - slice.trim_end().len());
    (trimmed_start < trimmed_end).then_some((trimmed_start, trimmed_end))
}

fn type_reference_from_text(
    type_text: &str,
    text_start: usize,
    text_end: usize,
    signature: &Signature,
) -> TypeReferenceSummary {
    let occurrence = occurrence_in_text_range(signature, text_start, text_end);
    let scip_symbol = occurrence.map(|occurrence| occurrence.symbol.clone());
    let confidence = if scip_symbol.is_some() {
        "scip_symbol"
    } else {
        "signature_text_only"
    };
    TypeReferenceSummary {
        display_text: type_text.to_owned(),
        scip_symbol,
        source_span: None,
        signature_range: occurrence
            .and_then(occurrence_range)
            .and_then(|range| signature_range_from_scip(&range)),
        confidence: confidence.to_owned(),
    }
}

fn occurrence_in_text_range(
    signature: &Signature,
    text_start: usize,
    text_end: usize,
) -> Option<&Occurrence> {
    signature.occurrences.iter().find(|occurrence| {
        if occurrence.symbol.is_empty() {
            return false;
        }
        let Some(range) = occurrence_range(occurrence) else {
            return false;
        };
        let Some((start_line, start_column, end_line, end_column)) = range_bounds(&range) else {
            return false;
        };
        if start_line != 0 || end_line != 0 {
            return false;
        }
        let Ok(start_column) = usize::try_from(start_column) else {
            return false;
        };
        let Ok(end_column) = usize::try_from(end_column) else {
            return false;
        };
        text_start <= start_column && end_column <= text_end
    })
}

fn signature_range_from_scip(range: &[i32]) -> Option<Vec<u32>> {
    let (start_line, start_column, end_line, end_column) = range_bounds(range)?;
    Some(vec![
        one_based_signature_position(start_line)?,
        one_based_signature_position(start_column)?,
        one_based_signature_position(end_line)?,
        one_based_signature_position(end_column)?,
    ])
}

fn one_based_signature_position(position: i32) -> Option<u32> {
    u32::try_from(position.checked_add(1)?).ok()
}

fn matching_close_paren(text: &str, open_index: usize) -> Option<usize> {
    let mut depth = 0_i32;
    for (index, character) in text
        .char_indices()
        .skip_while(|(index, _)| *index < open_index)
    {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_commas_with_offsets(
    text: &str,
    start: usize,
    end: usize,
) -> Vec<(usize, usize)> {
    let mut parts = Vec::new();
    let mut part_start = start;
    let mut angle_depth = 0_i32;
    let mut paren_depth = 0_i32;
    let mut bracket_depth = 0_i32;
    let Some(slice) = text.get(start..end) else {
        return parts;
    };
    for (relative_index, character) in slice.char_indices() {
        let index = start + relative_index;
        match character {
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            '(' => paren_depth += 1,
            ')' if paren_depth > 0 => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            ',' if angle_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                parts.push((part_start, index));
                part_start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push((part_start, end));
    parts
}
