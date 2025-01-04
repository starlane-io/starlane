    use crate::kind::{FoundationKind, PlatformKind,ProviderKind};


    pub struct BaseContext<P,F> where P: PlatformContext, F: FoundationContext {
        platform: Context<P>,
        foundation: Context<F>
    }

    ///
    impl <P,F> BaseContext<P,F> where P: PlatformContext, F: FoundationContext { }

    pub trait BaseSubStrataContext: Send+Sync  {
        type Kind: Send+Sync+?Sized;
    }

    /// a nice struct to wrap traits
    pub struct Context<C>(C) where C: BaseSubStrataContext;

    pub trait PlatformContext: BaseSubStrataContext<Kind=FoundationKind> { }

    pub trait FoundationContext: BaseSubStrataContext<Kind=PlatformKind> { }
