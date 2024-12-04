use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use crate::base::err::BaseErr;
use crate::base::kind::IKind;

pub struct Call;

pub struct FoundationErrBuilder<'z> {
    pub kind: Option<(&'static str, &'static str)>,
    pub config: Option<&'z (dyn Display + 'z)>,
    pub settings: Option<&'z (dyn Display + 'z)>,
}

impl<'z> Default for FoundationErrBuilder<'z> {
    fn default() -> Self {
        Self {
            kind: None,
            config: None,
            settings: None,
        }
    }
}

impl<'z> FoundationErrBuilder<'z> {
    pub fn kind(&mut self, kind: impl IKind) -> &mut Self {
        self.kind = Some((kind.category(), kind.as_str()));
        self
    }

    pub fn config(&mut self, config: &'z (dyn Display + 'z)) -> &mut Self {
        self.config = Some(config);
        self
    }

    pub fn settings(&mut self, settings: &'z (dyn Display + 'z)) -> &mut Self {
        self.settings = Some(settings);
        self
    }

    pub fn err(mut self, err: impl Display + 'z) -> BaseErr {
        let mut rtn = String::new();

        if let Some((cat, kind)) = self.kind {
            let fmt = format!("Foundation ERR {}::{} --> ", cat, kind);
            rtn.push_str(fmt.as_str());
        }
        let (action, verbose) = if let Some(config) = self.config {
            ("config".to_string(), config.to_string())
        } else if let Some(settings) = self.settings {
            ("settings".to_string(), settings.to_string())
        } else {
            ("<yaml>".to_string(), "<?>".to_string())
        };

        let fmt = format!("{}: \n```{}```\n", action, verbose);
        rtn.push_str(fmt.as_str());

        let err = format!("serde err: '{}'", err);
        rtn.push_str(err.as_str());
        BaseErr::Msg(rtn)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionRequest {
    pub title: String,
    pub description: String,
    pub items: Vec<ActionItem>,
}

impl ActionRequest {
    pub fn new(title: String, description: String) -> Self {
        Self {
            title,
            description,
            items: vec![],
        }
    }

    pub fn add(&mut self, item: ActionItem) {
        self.items.push(item);
    }

    pub fn print(&self) {}
}

impl Display for ActionRequest {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("ACTION REQUEST: ")?;
        f.write_str(&self.title)?;
        f.write_str("\n")?;
        f.write_str(&self.description)?;
        f.write_str("\n")?;
        f.write_str(format!("ITEMS: {} required action items...", self.items.len()).as_str())?;
        f.write_str("\n")?;
        for (index, item) in self.items.iter().enumerate() {
            f.write_str(format!("{} -> {}", index.to_string(), item.title).as_str())?;

            if let Some(ref web) = item.website {
                f.write_str("\n")?;
                f.write_str(format!(" more info: {}", web).as_str())?;
            }
            f.write_str("\n")?;
            f.write_str(item.details.as_str())?;
            if self.items.len() != index {
                f.write_str("\n")?;
            }
        }

        f.write_str("\n")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionItem {
    pub title: String,
    pub website: Option<String>,
    pub details: String,
}

impl ActionItem {
    pub fn new(title: String, details: String) -> Self {
        Self {
            title,
            details,
            website: None,
        }
    }

    pub fn with_website(&mut self, website: String) {
        self.website = Some(website);
    }

    pub fn print(vec: &Vec<Self>) {}
}

impl Display for ActionItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.title)?;
        f.write_str("\n")?;
        if let Some(website) = &self.website {
            f.write_str("more info: ")?;
            f.write_str(website)?;
            f.write_str("\n")?;
        };

        f.write_str(&self.details)?;
        f.write_str("\n")
    }
}

