use glib::SignalHandlerId;
use gtk4::{Box as GtkBox, Button, Label, ToggleButton};
use std::cell::RefCell;
use std::rc::Rc;

pub struct InfoPanel {
    pub container: GtkBox,
    pub title_label: Label,
    pub created_label: Label,
    pub maintainer_label: Label,
    pub permissions_label: Label,
    pub deps_label: Label,
    pub info_text: Label,
    pub url_button: Button,
    pub release_notes_button: Button,
    pub pkgbuild_button: Button,
    pub aur_scan_button: Button,
    pub ignore_button: ToggleButton,
    pub ignore_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
    pub current_url: Rc<RefCell<Option<String>>>,
    pub current_release_notes_url: Rc<RefCell<Option<String>>>,
    pub current_package: Rc<RefCell<Option<String>>>,
}
