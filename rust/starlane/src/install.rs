use crate::env::{config_save, config_save_new, context, context_dir, GlobalMode, STARLANE_GLOBAL_SETTINGS};
use crate::foundation::{Foundation, StandAloneFoundation};
use crate::shutdown::{panic_shutdown, shutdown};
use crate::{env, Database, PgRegistryConfig, StarlaneConfig, COLORS, VERSION};
use cliclack::log::{error, success, warning};
use cliclack::{clear_screen, confirm, input, intro, outro, outro_cancel, select, set_theme, spinner, Confirm, Input, ProgressBar, Select, Theme, ThemeState, Validate};
use colored::Colorize;
use lerp::Lerp;
use starlane::env::{Enviro, StdEnviro};
use std::fmt::Display;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use std::{io, thread};
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use console::style;
use nom::combinator::all_consuming;
use text_to_ascii_art::to_art;
use textwrap::Options;
use tokio::fs;
use starlane_space::space::err::ParseErrs;
use starlane_space::space::parse::{filename, path, to_string, var_case, VarCase};
use starlane_space::space::parse::util::{new_span, result};
use crate::registry::postgres::embed::PgEmbedSettings;

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
        {
            self.console.splash();
            println!("{}",self.console.center( "* I N S T A L L E R *"));
            println!();
            println!("{}",self.console.center(format!("context: '{}'   version: {}", context, VERSION.to_string())));
        }

        self.console.intro("INSTALL STARLANE")?;

        match env::config() {
            Ok(Some(_)) => {
                self.console.warning(format!("A valid starlane configuration already exists: '{}' this install process will overwrite the existing config", env::config_path()))?;
                self.console.newlines(1usize);
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
                    fs::remove_file(env::config_path()).await?;
                    self.console.success("config reset.\n")?;
                }
            }

            Err(err) => {
                self.console.warning(format!("An invalid (corrupted or out of date) starlane configuration already exists: '{}' the installation process will overwrite this config file.", env::config_path())).unwrap_or_default();
                self.console.newlines(1usize);
                let should_continue = confirm("Proceed with installation?").interact()?;
                if !should_continue {
                    self.console.outro("Starlane installation aborted by user.")?;
                    println!();
                    println!();
                    println!();
                    shutdown(0);
                } else {
                    fs::remove_file(env::config_path()).await?;
                    self.console.success("config deleted.")?;
                }
            }
            Ok(None) => {
                // there's no config so proceed with install
            }
        }


        self.console.long_delay();

        self.console.note("Foundation", "Starlane requires a Foundation in order to provision and manage various resources. If you aren't sure what to select just choose the first option: Local Standalone" );

        self.console.long_delay();
        let mut selector= self
            .console
            .select(r#"Select a Foundation:"#)

            .item(
                InstallType::Standalone,
                "Local Standalone",
                self.console.wrap("recommended for local development"),
            )
            .item(
                InstallType::ExistingPostgres,
                "Local with Existing Postgres Cluster",
                self.console
                    .wrap("Choose if ou already have a Postgres instance up and running"),
            );

        let selected = selector.interact()?;


        match selected {
            InstallType::Standalone => StandaloneInstaller::new(self.console.clone()).start().await,
            InstallType::ExistingPostgres => panic!(),
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
        spinner.stop("config saved");
        let foundation = StandAloneFoundation::new();

        match &config.registry {
            PgRegistryConfig::Embedded(db) => {
                let configurator = DbConfigurator::new(self.console.clone(),db.clone());
                let db = configurator.start().await?;
                let spinner = self.console.spinner();
                spinner.start("saving registry config...");
                let mut config = config.clone();
                config.registry = db;
                config_save(config)?;
                self.console.success("registry configuration saved")?;
            }
            PgRegistryConfig::External(_) => {}
        }

        foundation.install(&config).await?;
        spinner.stop("local postgres registry created.");
        Ok(())
    }
}

pub struct DbConfigurator {
    pub console: Console,
    pub config: Database<PgEmbedSettings>
}

impl DbConfigurator {
  pub fn new(console:Console, config: Database<PgEmbedSettings>) ->  Self {
      Self { config, console }
  }

  pub async fn start(self) -> Result<PgRegistryConfig, anyhow::Error> {
      self.console.section("POSTGRES REGISTRY CONFIGURATION", "choose postgres registry settings for this context [hit ENTER for defaults]");

      let mut config = self.config.clone();;

      let database: VarCase  = self.console.input("Database name:").default_input(self.config.database.as_str()).validate(|s:&String|{
          let span = new_span(s.as_str());
          match result(all_consuming(var_case)(span)) {
              Ok(_) => Ok(()),
              Err(err) => Err(err)
          }
      }).interact()?;

      config.database = database.to_string();

      let user: VarCase  = self.console.input("username:").default_input(self.config.user.as_str()).validate_interactively(|s:&String|{
          let span = new_span(s.as_str());
          match result(all_consuming(var_case)(span)) {
              Ok(_) => Ok(()),
              Err(err) => Err(err)
          }
      }).interact()?;

      config.settings.user = user.to_string();


      let password: VarCase  = self.console.input("password:").default_input(self.config.password.as_str()).validate_interactively(|s:&String|{
          let span = new_span(s.as_str());
          match result(all_consuming(var_case)(span)) {
              Ok(_) => Ok(()),
              Err(err) => Err(err)
          }
      }).interact()?;


      config.settings.password = password.to_string();

      let database_dir: PathBuf= self.console.input("Database directory:").default_input(self.config.database_dir(&context_dir()).display().to_string().as_str()).validate(|s:&String|{
          let span = new_span(s.as_str());
          match result(all_consuming(path)(span)) {
              Ok(_) => Ok(()),
              Err(err) => Err(err)
          }
      }).interact()?;


      config.settings.database_dir = Some(database_dir);

      Ok(PgRegistryConfig::Embedded(config))
  }
}

#[derive(Clone)]
pub struct Console {
    pub enviro: Arc<dyn Enviro>,
    pub theme: Arc<dyn Theme>
}

impl Console {
    pub fn new() -> Self {
        set_theme(StarlaneTheme());
        Self {
            enviro: Arc::new(StdEnviro::default()),
            theme: Arc::new(StarlaneTheme())
        }
    }

    /*
    fn clear(&self) -> io::Result<()> {
        //clear_screen()
        print!("\x1B[2J\x1B[1;1H");
        Ok(())
    }

     */


    pub fn clear(&self) -> Result<(), anyhow::Error> {
        /*
        use crossterm::{terminal::{ClearType, Clear}, QueueableCommand, cursor::{MoveTo, Hide}};
        let mut out = std::io::stdout();
        out.queue(Hide).unwrap();
        out.queue(Clear(ClearType::All)).unwrap();
        out.queue(MoveTo(0, 0)).unwrap();
        out.flush()?;
         */

        Ok(clear_screen()?)
    }

    fn splash(&self) {
        self.splash_with_params(1, 1, 50);
    }

    pub fn section(&self, prompt: impl Display, message: impl Display) -> io::Result<()> {
        self.info(style(format!(" {} ", prompt)).on_blue().black())?;

        let bar = console::Emoji("│", "|");
        let color = self.theme.bar_color(&ThemeState::Submit);
        let bar = color.apply_to(bar);

        let message = color.apply_to(self.wrap_indent(message, 1usize));
        println!( "{bar} {message}");
        self.newlines(1);
        Ok(())
    }




    pub fn info(&self, text: impl Display) -> io::Result<()> {
        let padding = 10usize;
        let size = self.enviro.term_width();
        let len = size - padding;
        let text = textwrap::wrap(text.to_string().as_str(), len).join("\n");
        cliclack::log::info(text)
    }

    pub fn input(&self, text: impl Display) -> Input {
        input(text)
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
        let newlines = message.to_string().chars().rev().filter(|c|*c == '\n').count();
        let size = self.enviro.term_width();
        let len = size - padding;
        let text = textwrap::wrap(message.to_string().as_str(), len).join("\n");
        cliclack::log::success(text);
        self.newlines(newlines);
        Ok(())
    }





    pub fn note(&self, prompt: impl Display, message: impl Display) -> io::Result<()> {
        let padding = 10usize;

        let size = self.enviro.term_width();
        let len = size - padding;
        let text = textwrap::wrap(message.to_string().as_str(), len).join("\n").to_string().trim().to_string();
        cliclack::note(prompt, text)
    }

    pub fn wrap(&self, text: impl Display) -> impl Display {
        let padding = 10usize;
        let size = self.enviro.term_width();
        let len = size - padding;
        textwrap::wrap(text.to_string().as_str(), len).join("\n")
    }


    pub fn wrap_indent(&self, text: impl Display, indent: usize) -> impl Display {
        let padding = 10usize;
        let width = self.enviro.term_width()- padding;
        let mut options = Options::new(width);
        let indent = " ".repeat(indent).to_string();
        options.initial_indent = indent.as_str();
        textwrap::wrap(text.to_string().as_str(), options).join("\n")
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
        let m = format!(" {} ", m).to_string();
        intro(style(m).on_blue().black())?;
        self.long_delay();
        Ok(())
    }

    pub fn outro(&self, m: impl Display) -> io::Result<()> {
        outro(m)?;
        self.long_delay();
        Ok(())
    }

    pub fn outro_cancel(&self, m: impl Display) -> io::Result<()> {
        self.long_delay();
        outro_cancel(m)?;
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
            let bar = console::Emoji("│", "|");
            self.theme.bar_color(&ThemeState::Active);
            let color = self.theme.bar_color(&ThemeState::Submit);
            let bar = color.apply_to(bar);
            println!( "{bar}" );
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

#[derive(Clone)]
pub struct StarlaneTheme();

impl Theme for StarlaneTheme{
    fn format_log(&self, text: &str, symbol: &str) -> String {
        self.format_log_with_spacing(text, symbol, false)
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
    Standalone,
    ExistingPostgres,
}
