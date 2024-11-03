use std::time::Duration;
use text_to_ascii_art::to_art;
use cliclack::{confirm, intro, outro, select, spinner, ProgressBar};
use cliclack::log::{error, success, warning};
use tokio::fs;
use std::fmt::Display;
use std::{io, thread};
use std::ops::Deref;
use lerp::Lerp;
use colored::Colorize;
use starlane::env::{Enviro, StdEnviro};
use crate::{env, StarlaneConfig, COLORS};
use crate::env::{context, GlobalMode, STARLANE_GLOBAL_SETTINGS};
use crate::foundation::{Foundation, StandAloneFoundation};
use crate::shutdown::{panic_shutdown, shutdown};





#[tokio::main]
pub async fn install() -> Result<(), anyhow::Error> {
    let installer = Installer::new();
    installer.start().await
}

pub struct Installer {
    pub console: Console
}

impl Installer {
    pub fn new() -> Installer {
        Self { console: Console::new() }
    }

    pub async fn start(self) -> Result<(), anyhow::Error> {
        let context = context();
        intro(format!("install context '{}'", context))?;
        let spinner = spinner();
        spinner.start("checking configuration");
        match env::config() {
            Ok(Some(_)) => {
                warning(format!("A valid starlane configuration already exists: '{}' this install process will overwrite the existing config", env::config_path()))?;
                let should_continue = confirm(format!("Overwrite: '{}'?", env::config_path())).interact()?;
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

        intro("Install Starlane")?;

        self.console.success("Select a foundation for this runner.  A foundation abstracts most infrastructure capabilities for starlane (provisioning servers, networking etc)")?;
        //note("Foundations", "Select a foundation for this runner.  A foundation abstracts most infrastructure capabilities for starlane (provisioning servers, networking etc)" )?;
        self.console.note("Standalone", "If you are using Starlane to develop on your local machine the `Standalone` foundation is recommended and will get you going with minimal hastles")?;

        self.console.long_delay();
        let selected = select(r#"Choose a Foundation:"#)
            .item(
                "Standalone",
                "Standalone",
                self.console.wrap("Standalone is recommended for local development and if you are just getting started"),
            )
            /*        .item(
                        "Docker",
                        "Install Starlane on Docker Desktop",
                        "~~~ Kubernetes install isn't actually working in this demo!",
                    )

             */
            .interact()?;

        match selected {
            "Standalone" => self.standalone_foundation().await,
            x => {
                error(format!("Sorry! this is just a proof of concept and the '{}' Foundation is not working just yet!", x))?;
                outro("Installation aborted because of lazy developers")?;
                panic_shutdown("Installation aborted because of lazy developers");
                Ok(())
            }
        }
    }

    async fn standalone_foundation(&self) -> Result<(), anyhow::Error> {
        let spinner = spinner();
        spinner.start("starting install");
        self.console.long_delay();
        async fn inner(this: &Installer, spin: &ProgressBar) -> Result<(), anyhow::Error> {
            spin.set_message("generating config");
            let config = StarlaneConfig::default();
            spin.stop("config generated");
            spin.set_message("saving config");
            env::config_save(config.clone())?;
            this.console.info("config saved.")?;
            spin.set_message("creating local postgres registry [this may take a while...]");
            this.console.warning("creating local postgres registry [this may take a while...]");
            let foundation = StandAloneFoundation::new();
            foundation.install(&config).await?;

            Ok(())
        }

        match inner(self, &spinner).await {
            Ok(()) => {
                spinner.stop("installation complete");
                Ok(())
            }
            Err(err) => {
                spinner.stop("installation failed");
                error(self.console.wrap(err.to_string().as_str())).unwrap_or_default();
                outro("Standalone installation failed").unwrap_or_default();
                shutdown(1);
                Err(err)
            }
        }
    }




}


pub struct Console {
   pub enviro: Box<dyn Enviro>
}

impl Console {
    pub fn new() -> Self {
        Self {
            enviro: Box::new(StdEnviro::default())
        }
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



    pub fn success(&self,  message: impl Display) -> io::Result<()> {
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
        cliclack::note(prompt,text)
    }

    pub fn wrap(&self, text: impl Display) -> impl Display {
        let padding = 10usize;
        let size = self.enviro.term_width();
        let len = size - padding;
        textwrap::wrap(text.to_string().as_str(), len).join("\n")
    }

    pub fn spinner(&self) ->  Spinner {
        Spinner::new()
    }

    pub fn splash_with_params(&self, pre: usize, post: usize, interval: u64) {
        let size = self.enviro.term_width();
        if size > self.splash_widest("*STARLANE*") {
            self.splash_with_params_and_banners(pre, post, interval, vec!["*STARLANE*"]);
        } else if size > self.splash_widest("STAR") {
            self.splash_with_params_and_banners(pre, post, interval, vec!["STAR","LANE"]);
        } else {
            let begin= COLORS;
            let end= (0xFF, 0xFF, 0xFF);
            let banner = "* S T A R L A N E *";
            let buffer = self.center(banner).len()-banner.len();
            print!("{}"," ".repeat(buffer));
            let count = banner.chars().count();
            for (index,c) in banner.chars().enumerate() {
                let progress = index as f32 / count  as f32;
                let r = (begin.0 as f32).lerp(end.0 as f32, progress) as u8;
                let g = (begin.1 as f32).lerp(end.1 as f32, progress) as u8;
                let b = (begin.2 as f32).lerp(end.2 as f32, progress) as u8;
                print!("{}", c.to_string().truecolor(r, g, b));
            }
            println!();

        }

    }

    fn center<S>( &self, input: S) -> String where S: AsRef<str>{
        let string = input.as_ref();
        let widest = self.widest(string);
        let term_width = StdEnviro::default().term_width();
        if term_width < widest {
            return string.to_string();
        }

        let mut rtn = String::new();
        for line in string.lines() {
            let count = line.chars().count();
            let col = (term_width/2)-(count/2);
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
        string.lines()
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

        println!("{}","\n".repeat(pre));
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

        println!("{}","\n".repeat(post));
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

    pub fn intro(&self, m: impl Display) -> io::Result<()>{
        intro(m)?;
        self.long_delay();
        Ok(())
    }

    pub fn outro(&self, m: impl Display) -> io::Result<()>{
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

    pub fn newlines(&self, len: usize, delay: u64 )  {
        for _ in 0..len {
            println!();
            self.delay();
        }
    }
}


pub struct Spinner {
    pub bar: ProgressBar
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            bar: spinner()
        }
    }
}

impl Deref for Spinner {
    type Target = ProgressBar;

    fn deref(&self) -> &Self::Target {
        std::thread::sleep( Duration::from_secs(1));
        &self.bar
    }
}
