#![allow(dead_code)]

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PrivacyFlags {
    pub(crate) hide_in_library: bool,
    pub(crate) mark_as_private: bool,
    pub(crate) overlay_data_deleted: bool,
}

impl PrivacyFlags {
    pub(crate) fn with_overlay_data_cleared(mut self) -> Self {
        self.overlay_data_deleted = true;
        self
    }
}
