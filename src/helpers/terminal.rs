use gio::Cancellable;
use glib::SpawnFlags;
use vte4::{PtyFlags, Terminal, TerminalExtManual};

pub fn spawn_terminal(terminal: &Terminal, args: Vec<&str>) {
    terminal.spawn_async(
        PtyFlags::DEFAULT,    // no special flags
        None,                 // default working directory
        &args,                // command arguments
        &[],                  // default environment
        SpawnFlags::DEFAULT,  // no special flags
        || {},                // child setup function
        -1,                   // timeout
        None::<&Cancellable>, // cancellable
        |result| {
            if let Err(e) = result {
                eprintln!("Failed to spawn terminal: {}", e);
            }
        },
    );
}
