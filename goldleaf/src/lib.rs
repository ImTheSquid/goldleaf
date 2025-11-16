pub use async_trait::async_trait;
pub use mongodb;
use mongodb::{Collection, Database};

pub use goldleaf_derive::CollectionIdentity;

#[async_trait]
pub trait CollectionIdentity {
    const COLLECTION: &'static str;

    async fn save(&self, db: &Database) -> Result<(), mongodb::error::Error>;
}

/// Procedural macro collection implementation (see `goldleaf_derive::collection_identity`)
pub trait AutoCollection {
    fn auto_collection<T: CollectionIdentity + Send + Sync>(&self) -> Collection<T>;
}

impl AutoCollection for Database {
    #[inline]
    fn auto_collection<T: CollectionIdentity + Send + Sync>(&self) -> Collection<T> {
        self.collection(T::COLLECTION)
    }
}
