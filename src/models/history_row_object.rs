use glib::Object;
use glib::subclass::prelude::*;
use std::cell::RefCell;

use crate::models::history_row::HistoryRow;

mod imp {
    use super::*;

    pub struct HistoryRowObject {
        pub data: RefCell<Option<HistoryRow>>,
    }

    impl Default for HistoryRowObject {
        fn default() -> Self {
            return Self {
                data: RefCell::new(None),
            };
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HistoryRowObject {
        const NAME: &'static str = "HistoryRowObject";
        type Type = super::HistoryRowObject;
    }

    impl ObjectImpl for HistoryRowObject {}
}

glib::wrapper! {
    pub struct HistoryRowObject(ObjectSubclass<imp::HistoryRowObject>);
}

impl HistoryRowObject {
    pub fn new(row: HistoryRow) -> Self {
        let obj: Self = Object::builder().build();
        obj.imp().data.replace(Some(row));
        return obj;
    }

    pub fn row(&self) -> HistoryRow {
        return self.imp().data.borrow().clone().expect("row not set");
    }
}
