pub mod portal {
    use crate::version::v0_0_1::util::uuid;
    use serde::{Deserialize, Serialize};
    use std::ops::Deref;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Exchanger<T> {
        pub id: String,
        pub item: T,
    }

    impl<T> Exchanger<T> {
        pub fn new(item: T) -> Self {
            Exchanger {
                id: uuid(),
                item,
            }
        }

        pub fn with<X>(self, item: X) -> Exchanger<X> {
            Exchanger { id: self.id, item }
        }
    }

    impl<T> Deref for Exchanger<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.item
        }
    }

}
