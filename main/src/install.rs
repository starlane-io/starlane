use starlane_base::env::{
    config_save, enviro, Enviro, GlobalMode, StdEnviro, STARLANE_GLOBAL_SETTINGS, STARLANE_HOME,
};

use crate::{COOL, ERR, IMPORTANT, OK, UNDERSTATED, VERSION};
use anyhow::anyhow;
use cliclack::log::{error, remark};
use cliclack::{
    clear_screen, confirm, input, intro, outro, outro_cancel, progress_bar, select, set_theme,
    spinner, Confirm, Input, ProgressBar, Select, Theme, ThemeState, Validate,
};
use colored::{Colorize, CustomColor};
use console::style;
use lerp::Lerp;
use serde::Serialize;
use starlane_base::env;
use starlane_hyperspace::shutdown::shutdown;
use starlane_space::particle::Status;
use std::fmt::Display;
use std::io::Write;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use std::{io, thread};
use text_to_ascii_art::to_art;
use textwrap::Options;
//use starlane::base::foundation::implementation::docker_daemon_foundation::DockerDaemonFoundation;

#[tokio::main]
pub async fn install(edit: bool) -> Result<(), anyhow::Error> {
    let installer = Installer::new(edit);
    installer.start().await
}

pub struct Installer {
    pub console: Console,
    pub edit: bool,
}

impl Installer {
    pub fn new(edit: bool) -> Installer {
        Self {
            console: Console::new(),
            edit,
        }
    }

    pub async fn start(self) -> Result<(), anyhow::Error> {
        let context = enviro();
        self.console.splash();
        println!("{}", self.console.center("* I N S T A L L E R *"));

        self.console.intro("INSTALL STARLANE")?;
        let version = VERSION.to_string();
        let home = STARLANE_HOME.to_string();
        self.console.note(
            "ENVIRONMENT",
            self.console.key_value(
                format!(
                    r#"   home: {home}
context: {context}
version: {version}
"#
                )
                .as_str(),
            ),
        )?;

        self.console.newlines(3);

        match env::config() {
            Ok(Some(_)) => {
                if !self.edit {
                    let msg = format!("A config for context '{}' already exists.  To overwrite run install with the --edit flag i.e. `{}`", context, "starlane install --edit".custom_color(self.console.theme.important()));
                    self.console.outro_err(msg.as_str())?;
                    Err(anyhow!("{}", msg))?;
                }
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
                    self.console.success("configuration reset.\n")?;
                }
            }
            Err(err) => {
                self.console.warning(format!("An invalid (corrupted or out of date) starlane configuration already exists: '{}' the installation process will overwrite this config file.", env::config_path())).unwrap_or_default();
                self.console.newlines(1usize);
                let should_continue = confirm("Proceed with installation?").interact()?;
                if !should_continue {
                    self.console
                        .outro("Starlane installation aborted by user.")?;
                    println!();
                    println!();
                    println!();
                    shutdown(0);
                } else {
                    let config = StarlaneConfig::default();
                    config_save(config)?;
                    self.console.success("configuration reset.\n")?;
                }
            }
            Ok(None) => {
                // there's no config so proceed with install
            }
        }

        self.console.long_delay();

        self.console.note("Foundation", "Starlane requires a Foundation in order to provision and manage various resources. If you aren't sure what to select just choose the first option: Local Standalone");

        self.console.long_delay();
        let mut selector = self
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
        self.console.note("topic", "A config...")?;
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
        todo!();
        /*
        let mut spinner = self.console.spinner();
        spinner.start("starting Standalone installer");
        self.console.long_delay();
        spinner.next("Standalone installer started.", "generating config");
        let config = config().unwrap_or_default().unwrap_or_default();
        spinner.next("config generated", "saving config");
        env::config_save(config.clone())?;
        spinner.stop("config saved");
        let foundation = DockerDaemonFoundation::new();

        let mut spinner = self.console.spinner();
        match &config.registry {
            PgRegistryConfig::Embedded(db) => {
                let configurator = DbConfigurator::new(self.console.clone(), db.clone());
                let db = configurator.start().await?;
                spinner.start(format!("saving registry config -> ({})", config_path()));
                let mut config = config.clone();
                config.registry = PgRegistryConfig::Embedded(db.clone());
                config_save(config.clone())?;
                spinner.next(
                    "registry configuration saved",
                    "creating registry data directory",
                );
                < < < < < < < Updated
                upstream
                tokio::fs::create_dir_all(db.settings.database_dir.unwrap_or_default()).await?;
                spinner.stop("data directory created successfully");
                == == == =
                tokio::fs::create_dir_all(db_dir.clone()).await?;
                spinner.next(format!("data directory created successfully: '{}'", db_dir.display()), "initializing registry");
                let db = foundation.provision_postgres_cluster(&config).await?;

                let logger = logger!(&Point::global_registry());
                let registry = PostgresRegistry::new2(config.registry.clone(), logger).await?;
                registry.setup().await?;
                spinner.stop("registry initialized");
            }
            PgRegistryConfig::External(_) => {
                panic!("not implemented yet")
                    >> >> >> > Stashed
                changes
            }
            PgRegistryConfig::External(_) => {}
        }

        let bar = self.console.progress_bar(100);
        bar.start("downloading postgres...");

        foundation.install_postgres(&config).await?;
        bar.stop("postgres download complete");
        spinner.stop("local postgres registry created.");
        Ok(())

         */
    }
}

/*
pub struct DbConfigurator {
    pub console: Console,
    pub config: Database<PostgresClusterConfig>,
}

impl DbConfigurator {
    pub fn new(console: Console, config: Database<PostgresClusterConfig>) -> Self {
        Self { config, console }
    }

    pub async fn start(self) -> Result<Database<PostgresClusterConfig>, anyhow::Error> {
        self.console.section_intro(
            "POSTGRES REGISTRY CONFIGURATION",
            "customize standalone postgres registry",
        )?;

        let mut cfg = self.config.clone();

        loop {
            let database = &cfg.database;
            let database_dir = cfg.database_dir(context()).display().to_string();
            let schema = &cfg.schema;
            let username = &cfg.settings.username;
            let password = &cfg.settings.password;
            let info = format!(
                r#"data_dir: {database_dir}
database: {database}
schema:   {schema}
username: {username}
password: {password}
"#
            );

            let info = self.console.key_value(info.as_str());
            self.console.note("DATABASE CONFIG", format!("{info}"))?;
            let customize = self
                .console
                .confirm("Would you like to make changes to the Database config?")
                .initial_value(false)
                .interact()?;

            if !customize {
                return Ok(cfg);
            }

            let database_dir: PathBuf = self
                .console
                .input("Database directory:")
                .default_input(
                    cfg.database_dir(&context_dir())
                        .display()
                        .to_string()
                        .as_str(),
                )
                .validate(|s: &String| {
                    let span = new_span(s.as_str());
                    match result(all_consuming(path)(span)) {
                        Ok(_) => Ok(()),
                        Err(err) => Err(err),
                    }
                })
                .interact()?;
            cfg.settings.database_dir = Some(database_dir);

            let database: VarCase = self
                .console
                .input("Database name:")
                .default_input(cfg.database.as_str())
                .validate(|s: &String| {
                    let span = new_span(s.as_str());
                    match result(all_consuming(var_case)(span)) {
                        Ok(_) => Ok(()),
                        Err(err) => Err(err),
                    }
                })
                .interact()?;
            cfg.database = database.to_string();

            let schema: VarCase = self
                .console
                .input("Schema name:")
                .default_input(cfg.schema.as_str())
                .validate(|s: &String| {
                    let span = new_span(s.as_str());
                    match result(all_consuming(var_case)(span)) {
                        Ok(_) => Ok(()),
                        Err(err) => Err(err),
                    }
                })
                .interact()?;

            cfg.schema = schema.to_string();

            let user: VarCase = self
                .console
                .input("username:")
                .default_input(cfg.username.as_str())
                .validate_interactively(|s: &String| {
                    let span = new_span(s.as_str());
                    match result(all_consuming(var_case)(span)) {
                        Ok(_) => Ok(()),
                        Err(err) => Err(err),
                    }
                })
                .interact()?;

            cfg.settings.username = user.to_string();

            let password: VarCase = self
                .console
                .input("password:")
                .default_input(cfg.password.as_str())
                .validate_interactively(|s: &String| {
                    let span = new_span(s.as_str());
                    match result(all_consuming(var_case)(span)) {
                        Ok(_) => Ok(()),
                        Err(err) => Err(err),
                    }
                })
                .interact()?;

            cfg.settings.password = password.to_string();

            self.console.section_success(
                "POSTGRES REGISTRY CONFIGURATION",
                "choose postgres registry settings for this context [hit ENTER for defaults]",
            )?;
        }
    }
}

 */

#[derive(Clone)]
pub struct Console {
    pub enviro: Arc<dyn Enviro>,
    pub theme: StarlaneTheme,
}

impl Console {
    pub fn new() -> Self {
        set_theme(StarlaneTheme());
        Self {
            enviro: Arc::new(StdEnviro::default()),
            theme: StarlaneTheme::default(),
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

    pub fn status<L>(&self, label: L, status: Status) -> io::Result<()>
    where
        L: AsRef<str>,
    {
        let theme = self.theme.clone();

        let color = match status {
            Status::Unknown => theme.under(),
            Status::Pending => theme.under(),
            Status::Init => theme.cool(),
            Status::Panic => theme.err(),
            Status::Fatal => theme.err(),
            Status::Ready => theme.ok(),
            Status::Paused => theme.under(),
            Status::Resuming => theme.cool(),
            Status::Done => theme.under(),
        };

        let status = status.to_string().custom_color(color);
        let label = label.as_ref().custom_color(theme.under());
        self.info(format!("{}: [{}]", label, status))
    }

    pub fn section_intro(&self, prompt: impl Display, message: impl Display) -> io::Result<()> {
        self.info(style(format!(" {} ", prompt)).on_blue().black())?;

        let bar = console::Emoji("│", "|");
        let color = self.theme.bar_color(&ThemeState::Submit);
        let bar = color.apply_to(bar);

        let message = color.apply_to(self.wrap_indent(message, 1usize));
        println!("{bar} {message}");
        self.newlines(1);
        Ok(())
    }

    pub fn section_success(&self, prompt: impl Display, message: impl Display) -> io::Result<()> {
        //self.success(style(format!(" {} ", prompt)).on_green().black())?;
        self.success(format!(" {} ", prompt))?;

        /*
        let bar = console::Emoji("│", "|");
        let color = self.theme.bar_color(&ThemeState::Submit);
        let bar = color.apply_to(bar);

        let message = color.apply_to(self.wrap_indent(message, 1usize));
        println!("{bar} {message}");

         */
        self.newlines(1);
        Ok(())
    }

    pub fn section_fail(&self, prompt: impl Display, message: impl Display) -> io::Result<()> {
        self.info(style(format!(" {} ", prompt)).on_red().black())?;

        let bar = console::Emoji("│", "|");
        let color = self.theme.bar_color(&ThemeState::Submit);
        let bar = color.apply_to(bar);

        let message = color.apply_to(self.wrap_indent(message, 1usize));
        println!("{bar} {message}");
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
        let newlines = message
            .to_string()
            .chars()
            .rev()
            .filter(|c| *c == '\n')
            .count();
        let size = self.enviro.term_width();
        let len = size - padding;
        let text = textwrap::wrap(message.to_string().as_str(), len).join("\n");
        cliclack::log::success(text);
        self.newlines(newlines);
        Ok(())
    }

    pub fn key_value(&self, string: &str) -> String {
        let key = self.theme.bar_color(&ThemeState::Submit);
        let value = self.theme.bar_color(&ThemeState::Active);
        let mut rtn = String::new();
        for line in string.lines() {
            if line.contains(":") {
                if let Some((k, v)) = line.split_once(":") {
                    let k = key.apply_to(k.to_string());
                    let v = value.apply_to(v.to_string());
                    let sep = key.apply_to(":");
                    rtn.push_str(k.to_string().as_str());
                    rtn.push_str(sep.to_string().as_str());
                    rtn.push_str(v.to_string().as_str());
                    rtn.push_str("\n");
                }
            } else {
                let k = key.apply_to(line.to_string());
                rtn.push_str(k.to_string().as_str());
                rtn.push_str("\n");
            }
        }

        rtn
    }

    pub fn note(&self, prompt: impl Display, message: impl Display) -> io::Result<()> {
        let padding = 10usize;

        let size = self.enviro.term_width();
        let len = size - padding;
        let text = textwrap::wrap(message.to_string().as_str(), len)
            .join("\n")
            .to_string()
            .trim()
            .to_string();
        cliclack::note(prompt, text)
    }

    pub fn error(&self, text: impl Display) -> io::Result<()> {
        error(text)
    }

    pub fn remark(&self, text: impl Display) -> io::Result<()> {
        remark(text)
    }

    pub fn wrap(&self, text: impl Display) -> impl Display {
        let padding = 10usize;
        let size = self.enviro.term_width();
        let len = size - padding;
        textwrap::wrap(text.to_string().as_str(), len).join("\n")
    }

    pub fn wrap_indent(&self, text: impl Display, indent: usize) -> impl Display {
        let padding = 10usize;
        let width = self.enviro.term_width() - padding;
        let mut options = Options::new(width);
        let indent = " ".repeat(indent).to_string();
        options.initial_indent = indent.as_str();
        textwrap::wrap(text.to_string().as_str(), options).join("\n")
    }

    pub fn spinner(&self) -> Spinner {
        Spinner::new(&self)
    }

    pub fn progress_bar(&self, len: u64) -> ProgressBar {
        progress_bar(len)
    }

    pub fn splash_with_params(&self, pre: usize, post: usize, interval: u64) {
        let size = self.enviro.term_width();
        if size > self.splash_widest("*STARLANE*") {
            self.splash_with_params_and_banners(pre, post, interval, vec!["*STARLANE*"]);
        } else if size > self.splash_widest("STAR") {
            self.splash_with_params_and_banners(pre, post, interval, vec!["STAR", "LANE"]);
        } else {
            let begin = COOL;
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
                    let end = COOL;

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
                let end = COOL;

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

    pub fn outro_err(&self, m: impl Display) -> io::Result<()> {
        outro_cancel(m)?;
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
            println!("{bar}");
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

impl Theme for StarlaneTheme {
    fn format_log(&self, text: &str, symbol: &str) -> String {
        self.format_log_with_spacing(text, symbol, false)
    }
}

impl StarlaneTheme {
    pub fn ok(&self) -> CustomColor {
        CustomColor::new(OK.0, OK.1, OK.2)
    }

    pub fn err(&self) -> CustomColor {
        CustomColor::new(ERR.0, ERR.1, ERR.2)
    }

    pub fn cool(&self) -> CustomColor {
        CustomColor::new(COOL.0, COOL.1, COOL.2)
    }

    pub fn under(&self) -> CustomColor {
        CustomColor::new(UNDERSTATED.0, UNDERSTATED.1, UNDERSTATED.2)
    }

    pub fn important(&self) -> CustomColor {
        CustomColor::new(IMPORTANT.0, IMPORTANT.1, IMPORTANT.2)
    }

    pub fn with_ok<R>(&self, string: R) -> String
    where
        R: AsRef<str>,
    {
        string.as_ref().truecolor(OK.0, OK.1, OK.2).to_string()
    }

    pub fn with_err<R>(&self, string: R) -> String
    where
        R: AsRef<str>,
    {
        string.as_ref().truecolor(ERR.0, ERR.1, ERR.2).to_string()
    }

    pub fn with_cool<R>(&self, string: R) -> String
    where
        R: AsRef<str>,
    {
        string
            .as_ref()
            .truecolor(COOL.0, COOL.1, COOL.2)
            .to_string()
    }

    pub fn with_under<R>(&self, string: R) -> String
    where
        R: AsRef<str>,
    {
        string
            .as_ref()
            .truecolor(UNDERSTATED.0, UNDERSTATED.1, UNDERSTATED.2)
            .to_string()
    }

    pub fn with_important<R>(&self, string: R) -> String
    where
        R: AsRef<str>,
    {
        string
            .as_ref()
            .truecolor(IMPORTANT.0, IMPORTANT.1, IMPORTANT.2)
            .to_string()
    }
}

impl Default for StarlaneTheme {
    fn default() -> Self {
        StarlaneTheme()
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

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}
