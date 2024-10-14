#![windows_subsystem = "windows"]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{env, fs::OpenOptions, process::Command, thread::sleep, time::Duration};
use streamdeck::{ImageOptions, StreamDeck};

fn main() -> Result<()> {
    let mut args = env::args();
    args.next();
    let vid = u16::from_str_radix(&args.next().unwrap(), 16)?;
    let pid = u16::from_str_radix(&args.next().unwrap(), 16)?;

    let config = parse_config()?;
    let active_page = &config.pages[0];
    let mut deck = init_streamdeck(vid, pid, &active_page, config.brightness)?;

    loop {
        let buttons_vec = match deck.read_buttons(None) {
            Ok(vec) => vec,
            Err(error) => {
                println!("Got error: {},", error);
                sleep(Duration::from_secs(1));
                deck = match init_streamdeck(vid, pid, &active_page, config.brightness) {
                    Ok(deck) => deck,
                    _ => deck,
                };

                continue;
            }
        };
        let index = match get_button_index(buttons_vec) {
            Some(index) => index,
            None => continue,
        };
        let button = match active_page.find_button(index) {
            Some(button) => button,
            None => continue,
        };

        if button.command[0].eq("page") {
            if let Some(page) = config
                .pages
                .iter()
                .find(|page| page.name == button.command[1])
            {
                reset_streamdeck(&mut deck, &page)?;
            }
        } else {
            let mut command = Command::new(&button.command[0]);
            command.args(&button.command[1..]);
            if let Some(work_dir) = &button.work_dir {
                command.current_dir(work_dir);
            }
            command.spawn()?;
        }
    }
}

fn init_streamdeck(vid: u16, pid: u16, page: &Page, brightness: u8) -> Result<StreamDeck> {
    let mut deck = StreamDeck::connect(vid, pid, None)?;

    deck.set_brightness(brightness)?;
    reset_streamdeck(&mut deck, page)?;

    Ok(deck)
}

fn reset_streamdeck(deck: &mut StreamDeck, page: &Page) -> Result<()> {
    deck.reset()?;
    for button in page.buttons.iter() {
        if let Some(image_path) = &button.image_path {
            deck.set_button_file(
                button.index as u8,
                image_path,
                &ImageOptions::new(None, false),
            )?;
        }
    }
    Ok(())
}

fn parse_config() -> Result<Config> {
    let file = OpenOptions::new().read(true).open("buttons.yaml")?;
    let config: Config = serde_yml::from_reader(file)?;
    Ok(config)
}

fn get_button_index(vec: Vec<u8>) -> Option<usize> {
    vec.iter().position(|i| *i == 1)
}

#[derive(Serialize, Deserialize)]
struct Button {
    index: usize,
    image_path: Option<String>,
    command: Vec<String>,
    work_dir: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Page {
    name: String,
    buttons: Vec<Button>,
}

#[derive(Serialize, Deserialize)]
struct Config {
    brightness: u8,
    pages: Vec<Page>,
}

impl Page {
    fn find_button(&self, index: usize) -> Option<&Button> {
        self.buttons.iter().find(|button| button.index == index)
    }
}
