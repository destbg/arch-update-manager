use gtk4::{Box as GtkBox, CheckButton, ListBoxRow};

pub struct ServiceRowState {
    pub row: ListBoxRow,
    pub name: String,
    pub check: CheckButton,
    pub status_box: GtkBox,
}
