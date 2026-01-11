use anyhow::Context;
use anyhow::Result;
use hidapi::HidApi;
use hidapi::HidDevice;
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use std::process::Command;
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use std::u64;

pub struct StreamDeck {
    device: HidDevice,
    config: Config,
    config_path: String,
    current_page: usize,
}

#[derive(Serialize, Deserialize)]
struct Config {
    brightness: u8,
    pages: Vec<Page>,
}

#[derive(Serialize, Deserialize)]
struct Page {
    buttons: Vec<Button>,
}

impl Page {
    fn find_button(&self, index: u8) -> Option<&Button> {
        self.buttons.iter().find(|button| button.index == index)
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct Button {
    index: u8,
    image_path: Option<String>,
    command: Vec<String>,
    work_dir: Option<String>,
}

impl StreamDeck {
    pub fn new(config_path: &str) -> Result<StreamDeck> {
        let device = wait_for_device().context("error connecting to device")?;
        device.set_blocking_mode(true)?;

        let config = load_config(config_path).context("error reading config")?;
        let deck = StreamDeck {
            device,
            config,
            config_path: config_path.to_string(),
            current_page: 0,
        };

        deck.apply_config()?;

        Ok(deck)
    }

    pub fn wait_for_input(&mut self) -> Result<u8> {
        loop {
            match self.get_button_presses() {
                Ok(buf) => match buf.iter().position(|&i| i == 1) {
                    Some(i) => return Ok(i as u8),
                    None => continue,
                },
                Err(error) => {
                    eprintln!("Error reading input: {}", error);
                    self.device = wait_for_device()?;
                    continue;
                }
            };
        }
    }

    pub fn execute_button_callback(&mut self, button_index: u8) -> Result<()> {
        let page = &self.config.pages[self.current_page];
        let button = page.find_button(button_index).unwrap();

        match button.command[0].as_str() {
            "page" => match button.command[1].as_str() {
                "next" => {
                    if self.current_page + 1 < self.config.pages.len() {
                        self.current_page += 1
                    }
                    self.apply_config()?;
                }
                "prev" => {
                    if self.current_page > 0 {
                        self.current_page -= 1
                    }
                    self.apply_config()?;
                }
                _ => {}
            },
            "reload" => {
                self.config = load_config(&self.config_path)?;
                self.current_page = 0;
                self.apply_config()?;
            }
            _ => {
                let button = button.clone();
                thread::spawn(move || {
                    let mut command = Command::new(&button.command[0]);
                    command.args(&button.command[1..]);
                    if let Some(work_dir) = &button.work_dir {
                        command.current_dir(work_dir);
                    }
                    match command.spawn() {
                        Ok(mut child) => {
                            let _ = child.wait();
                        }
                        Err(err) => {
                            eprintln!("{}", err);
                        }
                    }
                });
            }
        }

        Ok(())
    }

    fn apply_config(&self) -> Result<()> {
        self.reset().context("reset streamdeck")?;
        self.set_brightness().context("set brightness")?;

        for button in self.config.pages[self.current_page].buttons.iter() {
            if let Some(image_path) = &button.image_path {
                let data =
                    fs::read(image_path).context(format!("can't open image: {}", image_path))?;
                self.set_button_image(button.index, &data)?;
            }
        }

        Ok(())
    }

    fn reset(&self) -> Result<()> {
        let mut data = vec![3, 2];
        data.extend(vec![0; 30]);
        Ok(self.device.send_feature_report(&data)?)
    }

    fn set_brightness(&self) -> Result<()> {
        let mut data = vec![3, 8, self.config.brightness];
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
        }
    }
}

fn load_config(path: &str) -> Result<Config> {
    let file = fs::OpenOptions::new()
        .read(true)
        .open(path)
        .context("can't open config file")?;
    let config: Config = serde_yml::from_reader(file).context("can't parse config file")?;
    Ok(config)
}
