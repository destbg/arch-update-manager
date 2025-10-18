use glib::Object;
use glib::subclass::prelude::*;
use std::cell::RefCell;

use crate::models::package_update::PackageUpdate;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PackageUpdateObject {
        pub data: RefCell<PackageUpdate>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PackageUpdateObject {
        const NAME: &'static str = "PackageUpdateObject";
        type Type = super::PackageUpdateObject;
    }

    impl ObjectImpl for PackageUpdateObject {}
}

glib::wrapper! {
    pub struct PackageUpdateObject(ObjectSubclass<imp::PackageUpdateObject>);
}

impl PackageUpdateObject {
    pub fn new(data: PackageUpdate) -> Self {
        let obj: Self = Object::builder().build();
        obj.imp().data.replace(data);
        obj
    }

    pub fn data(&self) -> PackageUpdate {
        self.imp().data.borrow().clone()
    }

    pub fn set_selected(&self, selected: bool) {
        let mut data = self.imp().data.borrow_mut();
        data.selected = selected;
    }
}
