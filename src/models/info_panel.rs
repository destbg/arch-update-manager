use glib::SignalHandlerId;
use gtk4::{Box as GtkBox, Button, Label, ToggleButton};
use std::cell::RefCell;
use std::rc::Rc;

pub struct InfoPanel {
    pub container: GtkBox,
    pub info_text: Label,
    pub url_button: Button,
    pub ignore_button: ToggleButton,
    pub ignore_handler_id: Rc<RefCell<Option<SignalHandlerId>>>,
    pub current_url: Rc<RefCell<Option<String>>>,
    pub current_package: Rc<RefCell<Option<String>>>,
}
