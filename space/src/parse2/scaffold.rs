use crate::types2::archetype::Archetype;

pub struct PairScaffold<Prefix, Infix>
where
    Prefix: Archetype,
{
    prefix: Prefix,
    infix: Infix,
}
