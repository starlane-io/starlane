use std::str::FromStr;
use crate::space::parse::case::VarCase;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Variable {
    pub name: VarCase,
    pub trace: Trace,
}

impl Variable {
    pub fn new(name: VarCase, trace: Trace) -> Self {
        Self { name, trace }
    }
}

pub enum VarVal<V> {
    Var(Tw<VarCase>),
    Val(Tw<V>),
}

impl<V> TryInto<Variable> for VarVal<V> {
    type Error = ();

    fn try_into(self) -> Result<Variable, Self::Error> {
        match self {
            VarVal::Var(v) => {
                let var = Variable::new(v.w, v.trace);
                Ok(var)
            }
            VarVal::Val(_) => Err(()),
        }
    }
}

impl<V> ToResolved<V> for VarVal<V>
where
    V: FromStr<Err = ParseErrs>,
{
    fn to_resolved(self, env: &Env) -> Result<V, ParseErrs> {
        match self {
            VarVal::Var(var) => match env.val(var.as_str()) {
                Ok(val) => {
                    let val: String = val.clone().try_into()?;
                    Ok(V::from_str(val.as_str())?)
                }
                Err(err) => {
                    let trace = var.trace.clone();
                    match err {
                        ResolverErr::NotAvailable => Err(ParseErrs::from_range(
                            "variables not available in this context",
                            "variables not available",
                            trace.range,
                            trace.extra,
                        ).into()),
                        ResolverErr::NotFound => Err(ParseErrs::from_range(
                            format!("variable '{}' not found", var.unwrap().to_string()).as_str(),
                            "not found",
                            trace.range,
                            trace.extra,
                        ).into()),
                    }
                }
            },
            VarVal::Val(val) => Ok(val.unwrap()),
        }
    }
}