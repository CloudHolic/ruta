//! Handles: the only way anything outside the heap names an object.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StrRef(pub(super) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableRef(pub(super) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FuncRef(pub(super) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserDataRef(pub(super) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThreadRef(pub(super) u32);
