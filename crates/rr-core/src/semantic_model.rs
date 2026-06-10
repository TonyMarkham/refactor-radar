use crate::{
    CallEdgeSummary, ElementSummary, FileSummary, ProjectSummary, SourceSpan,
    SymbolReferenceSummary,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticModel {
    pub project: ProjectSummary,
    pub files: Vec<FileSummary>,
    pub elements: Vec<ElementSummary>,
    pub references: Vec<SymbolReferenceSummary>,
    pub call_edges: Vec<CallEdgeSummary>,
    pub spans: Vec<SourceSpan>,
}

impl SemanticModel {
    pub fn element_by_id(&self, id: &str) -> Option<&ElementSummary> {
        self.elements.iter().find(|element| {
            element.stable_id.as_str() == id
                || element.symbol_id.as_str() == id
                || element.scip_symbol.as_str() == id
        })
    }

    pub fn span_by_id(&self, id: &str) -> Option<&SourceSpan> {
        self.spans.iter().find(|span| span.id.as_str() == id)
    }

    pub fn has_project_id(&self, id: &str) -> bool {
        self.project.project_id == id
    }

    pub fn outgoing_call_edges(&self, id: &str) -> Vec<&CallEdgeSummary> {
        self.call_edges
            .iter()
            .filter(|edge| edge.enclosing_symbol_id.as_str() == id)
            .collect()
    }

    pub fn incoming_call_edges(&self, id: &str) -> Vec<&CallEdgeSummary> {
        self.call_edges
            .iter()
            .filter(|edge| edge.referenced_symbol_id.as_str() == id)
            .collect()
    }
}
