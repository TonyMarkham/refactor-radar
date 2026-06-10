use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    AssociatedType,
    Constant,
    Constructor,
    Enum,
    EnumMember,
    Field,
    File,
    Function,
    Macro,
    Method,
    MethodReceiver,
    Module,
    Namespace,
    Package,
    Parameter,
    SelfParameter,
    StaticMethod,
    StaticVariable,
    Struct,
    Trait,
    TraitMethod,
    Type,
    TypeAlias,
    TypeParameter,
    Union,
    Variable,
    Unknown(String),
}
