use rr_core::ElementKind;

use scip::types::symbol_information::Kind as ScipKind;

pub fn project_symbol_kind(kind: ScipKind) -> ElementKind {
    match kind {
        ScipKind::AssociatedType => ElementKind::AssociatedType,
        ScipKind::Attribute => ElementKind::Unknown("scip::Attribute".to_owned()),
        ScipKind::Constant => ElementKind::Constant,
        ScipKind::Enum => ElementKind::Enum,
        ScipKind::EnumMember => ElementKind::EnumMember,
        ScipKind::Field => ElementKind::Field,
        ScipKind::Function => ElementKind::Function,
        ScipKind::Macro => ElementKind::Macro,
        ScipKind::Method => ElementKind::Method,
        ScipKind::Module => ElementKind::Module,
        ScipKind::Parameter => ElementKind::Parameter,
        ScipKind::SelfParameter => ElementKind::SelfParameter,
        ScipKind::StaticMethod => ElementKind::StaticMethod,
        ScipKind::StaticVariable => ElementKind::StaticVariable,
        ScipKind::Struct => ElementKind::Struct,
        ScipKind::Trait => ElementKind::Trait,
        ScipKind::TraitMethod => ElementKind::TraitMethod,
        ScipKind::Type => ElementKind::Type,
        ScipKind::TypeAlias => ElementKind::TypeAlias,
        ScipKind::TypeParameter => ElementKind::TypeParameter,
        ScipKind::Union => ElementKind::Union,
        ScipKind::Variable => ElementKind::Variable,
        other => ElementKind::Unknown(format!("scip::{other:?}")),
    }
}
