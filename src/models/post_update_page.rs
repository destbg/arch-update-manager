use gtk4::{Box as GtkBox, Button};
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::section_visibility::SectionVisibility;

#[derive(Clone)]
pub struct PostUpdatePage {
    pub container: GtkBox,
    pub reboot_banner: GtkBox,
    pub loading_box: GtkBox,
    pub all_clear_box: GtkBox,
    pub sections_box: GtkBox,
    pub back_button: Button,
    pub section_visibility: Rc<RefCell<SectionVisibility>>,
}
