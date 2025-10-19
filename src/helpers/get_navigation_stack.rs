use gtk4::{ApplicationWindow, Box, Stack};
use vte4::{CastNone, GtkWindowExt, WidgetExt};

pub fn get_navigation_stack(widget: &impl WidgetExt) -> Option<(Stack, Box, ApplicationWindow)> {
    let Some(window) = widget.root().and_downcast::<ApplicationWindow>() else {
        return None;
    };
    let Some(main_container) = window.child().and_downcast::<Box>() else {
        return None;
    };
    let Some(stack) = main_container.first_child().and_downcast::<Stack>() else {
        return None;
    };
    let Some(content_box) = stack.child_by_name("content").and_downcast::<Box>() else {
        return None;
    };

    return Some((stack, content_box, window));
}
