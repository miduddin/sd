use anyhow::{Context, Result};
use std::env::args;
use streamdeck::StreamDeck;
mod streamdeck;

fn main() -> Result<()> {
    let config_path = args().nth(1).context("usage: sd /path/to/config.yaml")?;

    let mut deck = StreamDeck::new(&config_path)?;

    loop {
        let button = deck.wait_for_input()?;
        deck.execute_button_callback(button)?;
    }
}
