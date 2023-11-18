# Goldleaf

A thin wrapper over MongoDB to make it shine! Goldleaf uses struct field annotations to manage indexing, while also providing an `auto_collection` function for easy collection access.

Here's a small example of annotations:
```rust
#[derive(Serialize, Deserialize, CollectionIdentity, Debug, Default)]
#[db(name = "user")]
pub struct User {
    /// The unique ID of the user
    #[db(native_id_field)]
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ID>,
    /// The name of the user
    pub name: String,
    /// The unique username of the user
    #[db(indexing(index = 1, unique))]
    pub username: String,
    /// When the user was last active
    #[serde(with = "chrono_datetime_as_bson_datetime")]
    pub last_active: DateTime<Utc>,
    /// The user's email
    #[db(indexing(index = 1, unique, pfe = r#""email": {"$type": "string"}"#))]
    pub email: Option<String>,
    /// The user's phone number
    #[db(indexing(index = 1, unique))]
    pub phone_number: String,
    /// The user's sessions
    #[db(indexing(
        index = 1,
        unique,
        sub = "token",
        pfe = r#""sessions.token": {"$exists": true}"#
    ))]
    pub sessions: Vec<Session>,
}
```

Don't forget the annotation on your `id` field! Use `native_id_field` to prefix your ID field with an underscore. Otherwise, just use `id_field`.

To use the indices, call this function early in your code:
```rust
User::create_indices(&database).await?;
```
Note: This function will only exist if you create indices on your collection.

The API for MongoDB is mostly the same, but collection names are now statically-applied:
```rust
let users = db.auto_collection::<User>();
let username_matches = users
    .find_one(doc! {"username": &info.username}, None)
    .await?;
```

Given an instance of a struct, `save` can also be called:
```rust
user.save(&db).await?;
```
