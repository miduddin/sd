use anyhow::Context;
use anyhow::Result;
use hidapi::HidApi;
use hidapi::HidDevice;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::process::Command;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

#[derive(Deserialize)]
struct Config {
    brightness: u8,
    pages: Vec<ConfigPage>,
}

#[derive(Deserialize)]
struct ConfigPage {
    buttons: Vec<ConfigButton>,
}

#[derive(Deserialize)]
struct ConfigButton {
    index: u8,
    image_path: String,
    command: Vec<String>,
}

pub struct StreamDeck {
    device: HidDevice,
    config_path: String,
    pages: Vec<Page>,
    current_page: usize,
}

struct Page {
    buttons: HashMap<u8, Button>,
}

struct Button {
    image_path: String,
    action: Action,
}

enum Action {
    PagePrev,
    PageNext,
    Reload,
    Command(Vec<String>),
}

impl StreamDeck {
    pub fn new(config_path: String) -> Result<StreamDeck> {
        let mut deck = StreamDeck {
            device: wait_for_device().context("error connecting to device")?,
            config_path: config_path,
            pages: vec![],
            current_page: 0,
        };
        deck.load_config()?;
        Ok(deck)
    }

    pub fn wait_for_input(&mut self) -> Result<u8> {
        loop {
            match self.get_button_presses() {
                Ok(buf) => match buf.iter().position(|&i| i == 1) {
                    Some(i) => return Ok(i as u8),
                    None => (),
                },
                Err(error) => {
                    eprintln!("Error reading input: {}", error);
                    self.device = wait_for_device()?;
                }
            };
        }
    }

    pub fn execute_button_action(&mut self, button_index: u8) -> Result<()> {
        let button = self.pages[self.current_page].buttons.get(&button_index);
        if button.is_none() {
            return Ok(());
        }

        match button.unwrap().action {
            Action::PageNext => {
                if self.current_page + 1 < self.pages.len() {
                    self.current_page += 1
                }
                self.draw_current_page()?;
            }
            Action::PagePrev => {
                if self.current_page > 0 {
                    self.current_page -= 1
                }
                self.draw_current_page()?;
            }
            Action::Reload => {
                self.load_config()?;
            }
            Action::Command(ref arr) => {
                let mut command = Command::new(&arr[0]);
                command.args(&arr[1..]);
                thread::spawn(move || match command.spawn() {
                    Ok(mut child) => {
                        let _ = child.wait();
                    }
                    Err(err) => {
                        eprintln!("{}", err);
                    }
                });
            }
        }
        Ok(())
    }

    fn load_config(&mut self) -> Result<()> {
        let file = File::open(&self.config_path).context("can't open config file")?;
        let config: Config = yaml_serde::from_reader(file).context("can't parse config file")?;

        self.set_brightness(config.brightness)?;
        self.current_page = 0;
        self.pages = vec![];
        for config_page in config.pages {
            let mut page = Page {
                buttons: HashMap::new(),
            };
            for config_button in config_page.buttons {
                let action = match config_button.command[0].as_str() {
                    "page_prev" => Action::PagePrev,
                    "page_next" => Action::PageNext,
                    "reload" => Action::Reload,
                    _ => Action::Command(config_button.command),
                };
                page.buttons.insert(
                    config_button.index,
                    Button {
                        image_path: config_button.image_path,
                        action,
                    },
                );
            }
            self.pages.push(page);
        }
        self.draw_current_page()?;

        Ok(())
    }

    fn draw_current_page(&self) -> Result<()> {
        self.reset().context("reset streamdeck")?;

        for (index, button) in self.pages[self.current_page].buttons.iter() {
            let data = fs::read(&button.image_path)
                .context(format!("can't open image: {}", &button.image_path))?;
            self.set_button_image(*index, &data)?;
        }

        Ok(())
    }

    fn reset(&self) -> Result<()> {
        let mut data = vec![3, 2];
        data.extend(vec![0; 30]);
        Ok(self.device.send_feature_report(&data)?)
    }

    fn set_brightness(&self, brightness: u8) -> Result<()> {
        let mut data = vec![3, 8, brightness];
        data.extend(vec![0; 29]);
        Ok(self.device.send_feature_report(&data)?)
    }

    fn set_button_image(&self, button_index: u8, image_data: &[u8]) -> Result<()> {
        const WRITE_DATA_LEN: usize = 1024;
        const IMAGE_DATA_LEN: usize = WRITE_DATA_LEN - 8;

        let mut page_number = 0;
        let mut bytes_remaining = image_data.len();

        while bytes_remaining > 0 {
            let length = bytes_remaining.min(IMAGE_DATA_LEN);
            let bytes_sent = page_number * IMAGE_DATA_LEN;

            let mut buf: Vec<u8> = vec![
                2,
                7,
                button_index,
                if length == bytes_remaining { 1 } else { 0 },
                (length & 0xff) as u8,
                (length >> 8) as u8,
                (page_number & 0xff) as u8,
                (page_number >> 8) as u8,
            ];
            buf.extend(&image_data[bytes_sent..bytes_sent + length]);
            buf.extend(vec![0u8; WRITE_DATA_LEN - buf.len()]);

            self.device.write(&buf)?;

            bytes_remaining -= length;
            page_number += 1;
        }

        Ok(())
    }

    fn get_button_presses(&self) -> Result<Vec<u8>> {
        let mut buf = vec![0; 4 + 15];
        self.device.read(&mut buf)?;
        Ok(buf[4..].to_vec())
    }
}

fn wait_for_device() -> Result<HidDevice> {
    const VID_ELGATO: u16 = 0x0fd9;
    const PID_STREAMDECK_MK2: u16 = 0x0080;

    loop {
        let hid = HidApi::new().context("init hidapi")?;
        match hid.open(VID_ELGATO, PID_STREAMDECK_MK2) {
            Ok(device) => {
                device.set_blocking_mode(true)?;
                return Ok(device);
            }
            Err(error) => {
                eprintln!("{}", error);
                sleep(Duration::from_secs(3));
            }
        };
    }
}
