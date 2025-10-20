use vte4::TerminalExtManual;

pub fn spawn_terminal(terminal: &vte4::Terminal, args: Vec<&str>) {
    terminal.spawn_async(
        vte4::PtyFlags::DEFAULT,   // no special flags
        None,                      // default working directory
        &args,                     // command arguments
        &[],                       // default environment
        glib::SpawnFlags::DEFAULT, // no special flags
        || {},                     // child setup function
        -1,                        // timeout
        None::<&gio::Cancellable>, // cancellable
        |result| {
            if let Err(e) = result {
                eprintln!("Failed to spawn terminal: {}", e);
            }
        },
    );
}
