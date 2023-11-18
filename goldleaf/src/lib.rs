pub use async_trait::async_trait;
pub use mongodb;
use mongodb::{Database, Collection};

pub use goldleaf_derive::CollectionIdentity;

#[async_trait]
pub trait CollectionIdentity {
    const COLLECTION: &'static str;

    async fn save(&self, db: &Database) -> Result<(), mongodb::error::Error>;
}

/// Procedural macro collection implementation (see `goldleaf_derive::collection_identity`)
pub trait AutoCollection {
    fn auto_collection<T: CollectionIdentity>(&self) -> Collection<T>;
}

impl AutoCollection for Database {
    #[inline]
    fn auto_collection<T: CollectionIdentity>(&self) -> Collection<T> {
        self.collection(T::COLLECTION)
    }
}
