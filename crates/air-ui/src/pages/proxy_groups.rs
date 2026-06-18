pub(crate) mod fixtures;
mod format;
mod render;
mod runtime_projection;
mod state;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use format::group_sort_kind;
#[cfg(test)]
pub(crate) use format::{proxy_group_type_display_label, proxy_type_display_label};
pub use render::GroupNoticeLevel;
pub(crate) use render::{GroupPageInputs, render_groups_page};
#[cfg(test)]
pub(crate) use runtime_projection::{delay_color, displayed_member_delay};
pub use state::{GroupFormField, GroupListItem, GroupPageState};
