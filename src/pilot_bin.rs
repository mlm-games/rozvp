//! `rozvp-repose` binary: pilot shell entry (requires `repose-shell`).

fn main() -> anyhow::Result<()> {
    // Headless smoke without a window when asked; window otherwise.
    if std::env::args().any(|a| a == "--headless") {
        let (sun, zombies, peas, mowers) = rozvp::pilot::run_headless(2000);
        println!("pilot after 20 s: sun={sun} zombies={zombies} peas={peas} mowers={mowers}");
        return Ok(());
    }
    rozvp::pilot::runner::run()
}
