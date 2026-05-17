use gtk4::{Box as GtkBox, Button, Label};
use std::cell::RefCell;
use std::rc::Rc;

pub struct InfoPanel {
    pub container: GtkBox,
    pub info_text: Label,
    pub url_button: Button,
    pub current_url: Rc<RefCell<Option<String>>>,
}
