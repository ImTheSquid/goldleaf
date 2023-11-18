use goldleaf::CollectionIdentity;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, CollectionIdentity, Serialize, Deserialize)]
#[db(name = "test")]
struct Test {
    #[db(native_id_field)]
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    id: Option<ObjectId>,
    #[db(indexing(index = 1, unique))]
    username: String,
}

fn main() {
    // Make sure this exists
    let _ = Test::create_indices;
}
