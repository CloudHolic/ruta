//! Handles: the only way anything outside the heap names an object.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StrRef(pub(super) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableRef(pub(super) u32);

impl TableRef {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FuncRef(pub(super) u32);

impl FuncRef {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProtoRef(pub(super) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserDataRef(pub(super) u32);

impl UserDataRef {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThreadRef(pub(super) u32);

impl ThreadRef {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpvalRef(pub(super) u32);
