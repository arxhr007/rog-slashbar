//! rog-slashbar: ASUS ROG Slash bar as a Claude Code dashboard.
//!
//!   rog-slashbar daemon          run the renderer (auto-started by hooks / status line)
//!   rog-slashbar hook <state>    hook handler: start|working|waiting|done|ended
//!   rog-slashbar status          status line command (reads session JSON on stdin)
//!   rog-slashbar ctl <...>       status | view | set | stop
//!   rog-slashbar frames <...>    dev tool: dump rendered frames (used for the README GIFs)

mod bar;
mod ctl;
mod daemon;
mod demo;
mod hook;
mod render;
mod state;
mod status;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("daemon") => daemon::run(),
        Some("hook") => {
            // hooks must never fail or print anything
            let rest = args[1..].to_vec();
            let _ = std::panic::catch_unwind(move || hook::run(&rest));
        }
        Some("status") => {
            if std::panic::catch_unwind(status::run).is_err() {
                println!("slash status error");
            }
        }
        Some("ctl") => {
            if let Err(e) = ctl::run(&args[1..]) {
                eprintln!("{e}");
                std::process::exit(2);
            }
        }
        Some("frames") => {
            if let Err(e) = demo::run(&args[1..]) {
                eprintln!("{e}");
                std::process::exit(2);
            }
        }
        _ => {
            eprintln!("usage: rog-slashbar daemon | hook <state> | status | ctl <status|view|set|stop>");
            std::process::exit(2);
        }
    }
}
