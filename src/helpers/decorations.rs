pub fn are_decorations_disabled() -> bool {
    if let Some(settings) = gtk4::Settings::default() {
        if let Some(layout) = settings.gtk_decoration_layout() {
            if layout.is_empty() {
                return true;
            }
        }
    }

    return false;
}
