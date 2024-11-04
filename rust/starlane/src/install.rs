use crate::env::{context, GlobalMode, STARLANE_GLOBAL_SETTINGS};
use crate::foundation::{Foundation, StandAloneFoundation};
use crate::shutdown::{panic_shutdown, shutdown};
use crate::{env, StarlaneConfig, COLORS, VERSION};
use cliclack::log::{error, success, warning};
use cliclack::{clear_screen, confirm, intro, outro, select, spinner, Confirm, ProgressBar, Select};
use colored::Colorize;
use lerp::Lerp;
use starlane::env::{Enviro, StdEnviro};
use std::fmt::Display;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use std::{io, thread};
use text_to_ascii_art::to_art;
use tokio::fs;

#[tokio::main]
pub async fn install() -> Result<(), anyhow::Error> {
    let installer = Installer::new();
    installer.start().await
}

pub struct Installer {
    pub console: Console,
}

impl Installer {
    pub fn new() -> Installer {
        Self {
            console: Console::new(),
        }
    }

    pub async fn start(self) -> Result<(), anyhow::Error> {
        let context = context();
        intro(format!("install context '{}'", context))?;
        let spinner = spinner();
        spinner.start("checking configuration");
        match env::config() {
            Ok(Some(_)) => {
                warning(format!("A valid starlane configuration already exists: '{}' this install process will overwrite the existing config", env::config_path()))?;
                let should_continue = self
                    .console
                    .confirm(format!("Overwrite: '{}'?", env::config_path()))
                    .interact()?;
                if !should_continue {
                    outro("Starlane installation aborted by user.")?;
                    println!();
                    println!();
                    println!();
                    shutdown(0);
                } else {
                    spinner.start("deleting old config");
                    fs::remove_file(env::config_path()).await?;
                    self.console.info("config deleted.")?;
                    spinner.clear();
                }
            }

            Err(err) => {
                warning(format!("An invalid (corrupted or out of date) starlane configuration already exists: '{}' the installation process will overwrite this config file.", env::config_path())).unwrap_or_default();
                let should_continue = confirm("Proceed with installation?").interact()?;
                if !should_continue {
                    outro("Starlane installation aborted by user.")?;
                    println!();
                    println!();
                    println!();
                    shutdown(0);
                } else {
                    spinner.start("deleting invalid config");
                    fs::remove_file(env::config_path()).await?;
                    success("config deleted.")?;
                    spinner.clear();
                }
            }
            Ok(None) => {
                // there's no config so proceed with install
                spinner.clear();
            }
        }

        self.console.outro("checklist complete.")?;

        print!("{}", "\n".repeat(3));

        self.console.long_delay();

        self.console.clear()?;

        self.console.intro("Install Starlane")?;
        self.console.info( format!("version: {}", VERSION.to_string()) )?;
        self.console.info( format!("context: {}", context ) )?;
        self.console.newlines(3);
        self.console
            .info("Select a foundation for this run context.")?;
        //note("Foundations", "Select a foundation for this runner.  A foundation abstracts most infrastructure capabilities for starlane (provisioning servers, networking etc)" )?;

        self.console.long_delay();
        let selected = self
            .console
            .select(r#"Choose a Foundation:"#)

            .item(
                InstallType::Standalone,
                "Local Standalone Foundation",
                self.console.wrap("recommended for local development"),
            )
            .item(
                InstallType::ExistingPostgres,
                "Local with Existing Postgres Registry",
                self.console
                    .wrap("You already have a Postgres instance up and running"),
            )
            .item(
                InstallType::MoreInfo,
                "[more info...]",
                self.console
                    .wrap("Learn more about Starlane Foundations. Choose this one if you aren't sure"),
            )


            .interact()?;

        match selected {
            InstallType::Standalone => StandaloneInstaller::new(self.console.clone()).start().await,
            InstallType::ExistingPostgres => panic!(),
            InstallType::MoreInfo => self.foundation_more_info()
        }
    }

    fn foundation_more_info(&self) -> Result<(), anyhow::Error> {
        self.console.clear()?;
        self.console.intro("Foundation")?;
        self.console.note("topic","A foundation...")?;
        Ok(())
    }
}

struct StandaloneInstaller {
    pub console: Console,
}

impl StandaloneInstaller {
    pub fn new(console: Console) -> Self {
        Self { console }
    }
    async fn start(self) -> Result<(), anyhow::Error> {
        let mut spinner = self.console.spinner();
        spinner.start("starting install");
        self.console.long_delay();
        spinner.set_message("generating config");
        let config = StarlaneConfig::default();
        spinner.next("config generated", "saving config");
        env::config_save(config.clone())?;
        spinner.next("config saved", "creating local postgres registry");
        self.console
            .warning("creating local postgres registry [this may take a while...]")?;
        let foundation = StandAloneFoundation::new();
        foundation.install(&config).await?;
        spinner.stop("local postgres registry created.");
        Ok(())
    }
}

#[derive(Clone)]
pub struct Console {
    pub enviro: Arc<dyn Enviro>,
}

impl Console {
    pub fn new() -> Self {
        Self {
            enviro: Arc::new(StdEnviro::default()),
        }
    }

    fn clear(&self) -> io::Result<()> {
        clear_screen()
    }

    fn splash(&self) {
        self.splash_with_params(6, 6, 50);
    }

    pub fn info(&self, text: impl Display) -> io::Result<()> {
        let padding = 10usize;
        let size = self.enviro.term_width();
        let len = size - padding;
        let text = textwrap::wrap(text.to_string().as_str(), len).join("\n");
        cliclack::log::info(text)
    }

    pub fn warning(&self, text: impl Display) -> io::Result<()> {
        let padding = 10usize;
        let size = self.enviro.term_width();
        let len = size - padding;
        let text = textwrap::wrap(text.to_string().as_str(), len).join("\n");
        cliclack::log::warning(text)
    }

    pub fn success(&self, message: impl Display) -> io::Result<()> {
        let padding = 10usize;

        let size = self.enviro.term_width();
        let len = size - padding;
        let text = textwrap::wrap(message.to_string().as_str(), len).join("\n");
        cliclack::log::success(text)
    }

    pub fn note(&self, prompt: impl Display, message: impl Display) -> io::Result<()> {
        let padding = 10usize;

        let size = self.enviro.term_width();
        let len = size - padding;
        let text = textwrap::wrap(message.to_string().as_str(), len).join("\n");
        cliclack::note(prompt, text)
    }

    pub fn wrap(&self, text: impl Display) -> impl Display {
        let padding = 10usize;
        let size = self.enviro.term_width();
        let len = size - padding;
        textwrap::wrap(text.to_string().as_str(), len).join("\n")
    }

    pub fn spinner(&self) -> Spinner {
        Spinner::new(&self)
    }

    pub fn splash_with_params(&self, pre: usize, post: usize, interval: u64) {
        let size = self.enviro.term_width();
        if size > self.splash_widest("*STARLANE*") {
            self.splash_with_params_and_banners(pre, post, interval, vec!["*STARLANE*"]);
        } else if size > self.splash_widest("STAR") {
            self.splash_with_params_and_banners(pre, post, interval, vec!["STAR", "LANE"]);
        } else {
            let begin = COLORS;
            let end = (0xFF, 0xFF, 0xFF);
            let banner = "* S T A R L A N E *";
            let buffer = self.center(banner).len() - banner.len();
            print!("{}", " ".repeat(buffer));
            let count = banner.chars().count();
            for (index, c) in banner.chars().enumerate() {
                let progress = index as f32 / count as f32;
                let r = (begin.0 as f32).lerp(end.0 as f32, progress) as u8;
                let g = (begin.1 as f32).lerp(end.1 as f32, progress) as u8;
                let b = (begin.2 as f32).lerp(end.2 as f32, progress) as u8;
                print!("{}", c.to_string().truecolor(r, g, b));
            }
            println!();
        }
    }

    fn center<S>(&self, input: S) -> String
    where
        S: AsRef<str>,
    {
        let string = input.as_ref();
        let widest = self.widest(string);
        let term_width = StdEnviro::default().term_width();
        if term_width < widest {
            return string.to_string();
        }

        let mut rtn = String::new();
        for line in string.lines() {
            let count = line.chars().count();
            let col = (term_width / 2) - (count / 2);
            rtn.push_str(" ".repeat(col).as_str());
            rtn.push_str(line);
            rtn.push_str("\n");
        }
        rtn
    }

    fn splash_widest(&self, string: &str) -> usize {
        to_art(string.to_string(), "default", 0, 0, 0)
            .unwrap()
            .lines()
            .into_iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap()
    }

    fn widest(&self, string: &str) -> usize {
        string
            .lines()
            .into_iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap()
    }

    fn splash_with_params_and_banners(
        &self,
        pre: usize,
        post: usize,
        interval: u64,
        banners: Vec<&str>,
    ) {
        println!("{}", "\n".repeat(pre));
        for i in 0..banners.len() {
            let banner = banners.get(i).unwrap();
            match to_art(banner.to_string(), "default", 0, 0, 0) {
                Ok(string) => {
                    let string = self.center(string);
                    let begin = (0xFF, 0xFF, 0xFF);
                    let end = COLORS;

                    //let begin = (0x00, 0x00, 0x00);
                    // this is bad code however I couldn't find out how to get lines().len() withou
                    // giving up ownership (therefor the clone)
                    let size = string.clone().lines().count();

                    let mut index = 0;
                    for line in string.lines() {
                        let progress = index as f32 / size as f32;

                        //                    print!("{}",progress);

                        let r = (begin.0 as f32).lerp(end.0 as f32, progress) as u8;
                        let g = (begin.1 as f32).lerp(end.1 as f32, progress) as u8;
                        let b = (begin.2 as f32).lerp(end.2 as f32, progress) as u8;
                        println!("{}", line.truecolor(r, g, b));
                        self.delay();
                        index = index + 1;
                    }

                    //            println!("{}", string.truecolor(0xEE, 0xAA, 0x5A));
                }
                Err(err) => {
                    eprintln!("err! {}", err.to_string());
                }
            }
        }

        println!("{}", "\n".repeat(post));
    }

    pub fn splash2(&self) {
        self.splash();
    }

    fn splash_html(&self) {
        match to_art("*STARLANE*".to_string(), "default", 0, 0, 0) {
            Ok(string) => {
                let string = format!("\n\n\n\n\n\n{}\n\n\n\n\n\n", string).to_string();

                let begin = (0xFF, 0xFF, 0xFF);
                let end = (0xEE, 0xAA, 0x5A);
                let end = COLORS;

                //let begin = (0x00, 0x00, 0x00);
                // this is bad code however I couldn't find out how to get lines().len() withou
                // giving up ownership (therefor the clone)
                let size = string.clone().lines().count();
                let row_span = 1.0f32 / ((size - 10) as f32);
                let mut index = 0;
                for line in string.lines() {
                    let progress = if index < 5 {
                        0.0f32
                    } else if index > 6 && index < size - 5 {
                        (index - 5) as f32 / (size - 5) as f32
                    } else {
                        1.0f32
                    };
                    let r = (begin.0 as f32).lerp(end.0 as f32, progress) as u32;
                    let g = (begin.1 as f32).lerp(end.1 as f32, progress) as u32;
                    let b = (begin.2 as f32).lerp(end.2 as f32, progress) as u32;

                    let rgb = (r << 16) + (g << 8) + b;

                    let color = format!("{:#0x}", rgb);
                    println!("<div style=\"color:{}\">{}</div>", color, line);

                    index = index + 1;
                }

                //            println!("{}", string.truecolor(0xEE, 0xAA, 0x5A));
            }
            Err(err) => {
                eprintln!("err! {}", err.to_string());
            }
        }
    }

    pub fn intro(&self, m: impl Display) -> io::Result<()> {
        intro(m)?;
        self.long_delay();
        Ok(())
    }

    pub fn outro(&self, m: impl Display) -> io::Result<()> {
        outro(m)?;
        self.long_delay();
        Ok(())
    }

    pub fn delay(&self) {
        match &STARLANE_GLOBAL_SETTINGS.mode {
            GlobalMode::Newbie => {
                thread::sleep(Duration::from_millis(50));
            }
            GlobalMode::Expert => {}
        }
    }

    pub fn long_delay(&self) {
        match &STARLANE_GLOBAL_SETTINGS.mode {
            GlobalMode::Newbie => {
                thread::sleep(Duration::from_secs(1));
            }
            GlobalMode::Expert => {}
        }
    }

    pub fn newlines(&self, len: usize) {
        for _ in 0..len {
            println!();
            self.delay();
        }
    }

    pub fn confirm(&self, prompt: impl Display) -> Confirm {
        confirm(prompt)
    }

    pub fn select<T>(&self, prompt: impl Display) -> Select<T>
    where
        T: Clone + Eq,
    {
        select(prompt)
    }
}

pub struct Spinner<'a> {
    pub bar: ProgressBar,
    pub console: &'a Console,
}

impl<'a> Spinner<'a> {
    pub fn new(console: &'a Console) -> Self {
        Self {
            bar: spinner(),
            console,
        }
    }
}

impl<'a> Spinner<'a> {
    pub fn next(&mut self, stop: impl Display, start: impl Display) {
        let bar = &mut self.bar;
        bar.stop(stop);
        self.bar = spinner();
        self.bar.start(start);
        self.console.delay();
    }
}

impl<'a> Deref for Spinner<'a> {
    type Target = ProgressBar;

    fn deref(&self) -> &Self::Target {
        std::thread::sleep(Duration::from_secs(1));
        &self.bar
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum InstallType {
    MoreInfo,
    Standalone,
    ExistingPostgres,
}
