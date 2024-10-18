use std::sync::Arc;
use std::collections::HashMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use crate::space::err::ParseErrs;
use crate::space::parse::{CtxResolver, ResolverErr, VarResolver};
use crate::space::point::Point;
use crate::space::substance::{Bin, Substance};

#[derive(Clone, Serialize, Deserialize)]
pub struct File {
    pub name: String,
    pub content: Bin,
}

impl File {
    pub fn new<S: ToString>(name: S, content: Bin) -> Self {
        Self {
            name: name.to_string(),
            content,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileResolver {
    pub files: HashMap<String, Bin>,
}

impl FileResolver {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    pub fn file<N: ToString>(&self, name: N) -> Result<File, ResolverErr> {
        if let Some(content) = self.files.get(&name.to_string()) {
            Ok(File::new(name, content.clone()))
        } else {
            Err(ResolverErr::NotFound)
        }
    }

    /// grab the only file
    pub fn singleton(&self) -> Result<File, ResolverErr> {
        if self.files.len() == 1 {
            let i = &mut self.files.iter();
            if let Some((name, content)) = i.next() {
                Ok(File::new(name.clone(), content.clone()))
            } else {
                Err(ResolverErr::NotFound)
            }
        } else {
            Err(ResolverErr::NotFound)
        }
    }
}

impl Default for Env {
    fn default() -> Self {
        Self {
            parent: None,
            point: Point::root(),
            vars: HashMap::new(),
            file_resolver: FileResolver::new(),
            var_resolvers: MultiVarResolver::new(),
        }
    }
}

#[derive(Clone)]
pub struct CompositeResolver {
    pub env_resolver: Arc<dyn VarResolver>,
    pub scope_resolver: MapResolver,
    pub other_resolver: MultiVarResolver,
}

impl CompositeResolver {
    pub fn new() -> Self {
        Self {
            env_resolver: Arc::new(NoResolver::new()),
            scope_resolver: MapResolver::new(),
            other_resolver: MultiVarResolver::new(),
        }
    }

    pub fn set<S>(&mut self, key: S, value: Substance)
    where
        S: ToString,
    {
        self.scope_resolver.insert(key.to_string(), value);
    }
}

impl VarResolver for CompositeResolver {
    fn val(&self, var: &str) -> Result<Substance, ResolverErr> {
        if let Ok(val) = self.scope_resolver.val(var) {
            Ok(val)
        } else if let Ok(val) = self.scope_resolver.val(var) {
            Ok(val)
        } else if let Ok(val) = self.other_resolver.val(var) {
            Ok(val)
        } else {
            Err(ResolverErr::NotFound)
        }
    }
}

pub struct PointCtxResolver(Point);

impl CtxResolver for PointCtxResolver {
    fn working_point(&self) -> Result<&Point, ParseErrs> {
        Ok(&self.0)
    }
}

#[derive(Clone)]
pub struct NoResolver;

impl NoResolver {
    pub fn new() -> Self {
        Self {}
    }
}

impl VarResolver for NoResolver {}

#[derive(Clone)]
pub struct MapResolver {
    pub map: HashMap<String, Substance>,
}

impl MapResolver {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert<K: ToString>(&mut self, key: K, value: Substance) {
        self.map.insert(key.to_string(), value);
    }
}

impl VarResolver for MapResolver {
    fn val(&self, var: &str) -> Result<Substance, ResolverErr> {
        self.map
            .get(&var.to_string())
            .cloned()
            .ok_or(ResolverErr::NotFound)
    }
}

#[derive(Clone)]
pub struct RegexCapturesResolver {
    regex: Regex,
    text: String,
}

impl RegexCapturesResolver {
    pub fn new(regex: Regex, text: String) -> Result<Self, ParseErrs> {
        regex.captures(text.as_str()).ok_or(ParseErrs::new("no regex captures"))?;
        Ok(Self { regex, text })
    }
}

impl VarResolver for RegexCapturesResolver {
    fn val(&self, id: &str) -> Result<Substance, ResolverErr> {
        let captures = self
            .regex
            .captures(self.text.as_str())
            .expect("expected captures");
        match captures.name(id) {
            None => Err(ResolverErr::NotFound),
            Some(m) => Ok(Substance::Text(m.as_str().to_string())),
        }
    }
}

#[derive(Clone)]
pub struct MultiVarResolver(Vec<Arc<dyn VarResolver>>);

impl Default for MultiVarResolver {
    fn default() -> Self {
        MultiVarResolver::new()
    }
}

impl MultiVarResolver {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn push(&mut self, resolver: Arc<dyn VarResolver>) {
        self.0.push(resolver);
    }
}

impl VarResolver for MultiVarResolver {
    fn val(&self, var: &str) -> Result<Substance, ResolverErr> {
        for resolver in &self.0 {
            match resolver.val(&var.to_string()) {
                Ok(ok) => return Ok(ok),
                Err(_) => {}
            }
        }
        Err(ResolverErr::NotFound)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ctx {
    WorkingPoint,
    PointFromRoot,
}

impl ToString for Ctx {
    fn to_string(&self) -> String {
        match self {
            Ctx::WorkingPoint => ".".to_string(),
            Ctx::PointFromRoot => "...".to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Env {
    parent: Option<Box<Env>>,
    pub point: Point,
    pub vars: HashMap<String, Substance>,
    pub file_resolver: FileResolver,

    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default)]
    pub var_resolvers: MultiVarResolver,
}

impl Env {
    pub fn new(working: Point) -> Self {
        Self {
            parent: None,
            point: working,
            vars: HashMap::new(),
            file_resolver: FileResolver::new(),
            var_resolvers: MultiVarResolver::new(),
        }
    }

    pub fn no_point() -> Self {
        Self::new(Point::root())
    }

    pub fn push(self) -> Self {
        Self {
            point: self.point.clone(),
            parent: Some(Box::new(self)),
            vars: HashMap::new(),
            file_resolver: FileResolver::new(),
            var_resolvers: MultiVarResolver::new(),
        }
    }

    pub fn push_working<S: ToString>(self, segs: S) -> Result<Self, ParseErrs> {
        Ok(Self {
            point: self.point.push(segs.to_string())?,
            parent: Some(Box::new(self)),
            vars: HashMap::new(),
            file_resolver: FileResolver::new(),
            var_resolvers: MultiVarResolver::new(),
        })
    }

    pub fn point_or(&self) -> Result<Point, ParseErrs> {
        Ok(self.point.clone())
    }

    pub fn pop(self) -> Result<Env, ParseErrs> {
        Ok(*self
            .parent
            .ok_or(ParseErrs::new(&"expected parent scopedVars"))?)
    }

    pub fn add_var_resolver(&mut self, var_resolver: Arc<dyn VarResolver>) {
        self.var_resolvers.push(var_resolver);
    }

    pub fn val<K: ToString>(&self, var: K) -> Result<Substance, ResolverErr> {
        match self.vars.get(&var.to_string()) {
            None => {
                if let Ok(val) = self.var_resolvers.val(var.to_string().as_str()) {
                    Ok(val.clone())
                } else if let Some(parent) = self.parent.as_ref() {
                    parent.val(var.to_string())
                } else {
                    Err(ResolverErr::NotFound)
                }
            }
            Some(val) => Ok(val.clone()),
        }
    }

    pub fn set_working(&mut self, point: Point) {
        self.point = point;
    }

    pub fn working(&self) -> &Point {
        &self.point
    }

    pub fn set_var_str<V: ToString>(&mut self, key: V, value: V) {
        self.vars
            .insert(key.to_string(), Substance::Text(value.to_string()));
    }

    pub fn set_var<V: ToString>(&mut self, key: V, value: Substance) {
        self.vars.insert(key.to_string(), value);
    }

    pub fn file<N: ToString>(&self, name: N) -> Result<File, ResolverErr> {
        match self.file_resolver.files.get(&name.to_string()) {
            None => {
                if let Some(parent) = self.parent.as_ref() {
                    parent.file(name.to_string())
                } else {
                    Err(ResolverErr::NotFound)
                }
            }
            Some(bin) => Ok(File::new(name.to_string(), bin.clone())),
        }
    }

    pub fn set_file<N: ToString>(&mut self, name: N, content: Bin) {
        self.file_resolver.files.insert(name.to_string(), content);
    }
}