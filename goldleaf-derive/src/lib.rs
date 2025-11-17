#![deny(clippy::unwrap_used)]

use proc_macro::TokenStream;

use darling::util::Flag;
use darling::{ast, FromDeriveInput, FromField, FromMeta};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[derive(FromMeta, Copy, Clone)]
enum TwoD {
    Spherical,
    Cartesian,
}

#[derive(FromMeta, Default)]
struct FieldIdentityMetaData {
    /// Subfield for multikey
    sub: Option<String>,
    /// The index number
    index: Option<i32>,
    /// Links to another index, creating a compound
    /// Must be format <name>
    link: Option<String>,
    /// The link order
    order: Option<u8>,
    unique: Flag,
    /// Automatically sets to be text index
    text_weight: Option<u8>,
    two_d: Option<TwoD>,
    /// Flags a field as containing the locale information
    icase_locale: Option<String>,
    icase_strength: Option<u8>,
    name: Option<String>,
    two_d_bits: Option<u32>,
    two_d_max: Option<f64>,
    two_d_min: Option<f64>,
    lang_field: Flag,
    pfe: Option<String>,
}

const DEFAULT_2D_BITS: u32 = 26;
const DEFAULT_2D_MIN: f64 = -180.0;
const DEFAULT_2D_MAX: f64 = 180.0;

#[derive(FromField)]
#[darling(attributes(db))]
struct FieldIdentityData {
    ident: Option<syn::Ident>,
    id_field: Flag,
    native_id_field: Flag,
    indexing: Option<FieldIdentityMetaData>,
}

#[derive(Default, Copy, Clone)]
enum TwoDPacked {
    Spherical {
        bits: u32,
        max: f64,
        min: f64,
    },
    #[default]
    Cartesian,
}

struct CombinedFieldIdentityData {
    ident: Option<syn::Ident>,
    /// Subfield for multikey
    sub: Option<String>,
    /// The index number
    index: Option<i32>,
    /// Links to another index, creating a compound
    /// Must be format <name>
    link: Option<String>,
    /// The link order
    order: Option<u8>,
    unique: Flag,
    /// Automatically sets to be text index
    text_weight: Option<u8>,
    two_d: Option<TwoDPacked>,
    icase_locale: Option<String>,
    icase_strength: Option<u8>,
    name: Option<String>,
    lang_field: Flag,
    /// Partial filter expression
    pfe: Option<String>,
}

impl From<FieldIdentityData> for CombinedFieldIdentityData {
    fn from(value: FieldIdentityData) -> Self {
        let meta = value.indexing.expect("Indexing metadata");
        CombinedFieldIdentityData {
            ident: value.ident,
            sub: meta.sub,
            index: meta.index,
            link: meta.link,
            order: meta.order,
            unique: meta.unique,
            text_weight: meta.text_weight,
            two_d: match meta.two_d {
                None => None,
                Some(two_d) => Some(match two_d {
                    TwoD::Spherical => TwoDPacked::Spherical {
                        bits: meta.two_d_bits.unwrap_or(DEFAULT_2D_BITS),
                        max: meta.two_d_max.unwrap_or(DEFAULT_2D_MAX),
                        min: meta.two_d_min.unwrap_or(DEFAULT_2D_MIN),
                    },
                    TwoD::Cartesian => TwoDPacked::Cartesian,
                }),
            },
            icase_locale: meta.icase_locale,
            icase_strength: meta.icase_strength,
            name: meta.name,
            lang_field: meta.lang_field,
            pfe: meta.pfe,
        }
    }
}

enum IndexType {
    Numeric(i32),
    Text(u32),
    TwoD(TwoDPacked),
}

struct IndexPair {
    ident: String,
    index: IndexType,
    order_index: u8,
    is_lang_field: bool,
}

#[derive(Default)]
struct CaseInsensitivity {
    locale: String,
    strength: u8,
}

struct CollatedFieldIdentityData {
    pairs: Vec<IndexPair>,
    unique: bool,
    name: Option<String>,
    link: Option<String>,
    case_insensitivity: Option<CaseInsensitivity>,
    two_d: Option<TwoDPacked>,
    /// Partial filter expression
    pfe: Option<String>,
}

#[derive(FromDeriveInput)]
#[darling(attributes(db), supports(struct_named), forward_attrs(allow, doc, cfg))]
struct CollectionIdentityData {
    ident: syn::Ident,
    name: String,
    /// When the document will expire, in seconds
    expiration_secs: Option<u64>,
    data: ast::Data<(), FieldIdentityData>,
}

#[proc_macro_derive(CollectionIdentity, attributes(db))]
pub fn collection_identity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let collection = match CollectionIdentityData::from_derive_input(&input) {
        Ok(parsed) => parsed,
        Err(e) => return e.write_errors().into(),
    };

    let collection_name = collection.name;
    let struct_name = collection.ident;

    // Generate indexing if necessary
    let mut fields = collection
        .data
        .take_struct()
        .expect("Must be struct")
        .fields;

    let mut id_field = None;
    let mut native_id = false;

    for field in &mut fields {
        if field.id_field.is_present() || field.native_id_field.is_present() {
            if id_field.is_some()
                || (field.id_field.is_present() && field.native_id_field.is_present())
            {
                panic!("Multiple ID fields not allowed!");
            }

            id_field = Some(
                field
                    .ident
                    .as_ref()
                    .expect("ID field identifier")
                    .to_string(),
            );
            native_id = field.native_id_field.is_present();

            if !native_id {
                field.indexing.get_or_insert_default().unique = Flag::present();
            }
        }
    }

    let id_field = id_field.expect("ID field must be present!");
    let id_field_tok: syn::Ident = syn::parse_str(&id_field).expect("Valid parse of ID field");
    let (id_field, id_field_value) = if native_id {
        (
            format!("_{id_field}"),
            quote!(self.#id_field_tok.as_ref().unwrap()),
        )
    } else {
        (id_field, quote!(&self.#id_field_tok))
    };

    // Generate collection identity
    let identity = quote! {
        #[::goldleaf::async_trait]
        impl ::goldleaf::CollectionIdentity for #struct_name {
            const COLLECTION: &'static str = #collection_name;

            async fn save(&self, db: &::goldleaf::mongodb::Database) -> Result<(), ::mongodb::error::Error> {
                let coll = <::goldleaf::mongodb::Database as ::goldleaf::AutoCollection>::auto_collection::<Self>(db);
                let res = coll.replace_one(::goldleaf::mongodb::bson::doc! {
                    #id_field: #id_field_value
                }, self).await?;

                debug_assert_eq!(res.matched_count, 1, "unable to find structure with identifying field `{}`", #id_field);

                Ok(())
            }
        }
    };

    let indexing_fields = fields
        .into_iter()
        .filter(|f| f.indexing.is_some())
        .collect::<Vec<_>>();
    if indexing_fields.is_empty() {
        return identity.into();
    }
    let indexing_fields = indexing_fields
        .into_iter()
        .map(CombinedFieldIdentityData::from)
        .collect::<Vec<_>>();

    // Collate indices into a more readable struct
    let mut identities: Vec<CollatedFieldIdentityData> = vec![];
    for field in indexing_fields {
        // If this field is linked, try to find other fields with the same link ID and combine them
        if let Some(link_id) = &field.link {
            if let Some(id) = identities
                .iter_mut()
                .find(|id| id.link.as_ref().is_some_and(|l| l == link_id))
            {
                id.pairs.push(generate_index_pair(&field));

                id.pairs.sort_unstable_by_key(|data| data.order_index);

                id.unique = id.unique || field.unique.is_present();
                if let Some(name) = field.name {
                    id.name = Some(name);
                }
                if let (Some(locale), Some(strength)) = (field.icase_locale, field.icase_strength) {
                    id.case_insensitivity = Some(CaseInsensitivity { locale, strength })
                }

                if let Some(two_d) = field.two_d {
                    id.two_d = Some(two_d);
                }
            }
        } else {
            // This field is independent, just generate identity data separately
            identities.push(CollatedFieldIdentityData {
                pairs: vec![generate_index_pair(&field)],
                unique: field.unique.is_present(),
                name: field.name,
                link: field.link,
                case_insensitivity: if let (Some(locale), Some(strength)) =
                    (field.icase_locale, field.icase_strength)
                {
                    Some(CaseInsensitivity { locale, strength })
                } else {
                    None
                },
                two_d: field.two_d,
                pfe: field.pfe,
            })
        }
    }

    // Generate doc strings
    let docs = identities
        .iter()
        .map(|i| {
            let pairs = i.pairs.iter().map(|p| {
                let ident = p.ident.clone();
                match &p.index {
                    IndexType::Numeric(val) => quote! {
                        #ident: #val
                    },
                    IndexType::Text { .. } => quote! {
                        #ident: "text"
                    },
                    IndexType::TwoD(two_d) => match two_d {
                        TwoDPacked::Spherical { .. } => quote! {
                            #ident: "2dsphere"
                        },
                        TwoDPacked::Cartesian => quote! {
                            #ident: "2d"
                        },
                    },
                }
            });

            quote! {
                ::goldleaf::mongodb::bson::doc!{#(#pairs),*}
            }
        })
        .collect::<Vec<_>>();

    // Generate builder strings
    let opts = identities.iter().map(|i| {
        let index_name = i.name.clone().unwrap_or("".to_string());
        let unique = i.unique;

        // Figure out if any index pair has this info
        // MULTIDIMENSIONAL SEARCHING
        let use_two_d = i.two_d.is_some_and(|t| match t {
            TwoDPacked::Spherical { .. } => true,
            TwoDPacked::Cartesian => false,
        });
        let two_d = i.two_d.unwrap_or_default();
        let (bits, max, min) = match two_d {
            TwoDPacked::Spherical { bits, max, min } => (bits, max, min),
            TwoDPacked::Cartesian => (0, 0f64, 0f64),
        };

        // TEXT WEIGHTS
        let pairs = i.pairs.iter().filter_map(|p| match p.index {
            IndexType::Text(weight) => Some((p, weight)),
            _ => None,
        }).map(|(text_pair, weight)| {
            let ident = text_pair.ident.clone();
            quote! { #ident: #weight }
        }).collect::<Vec<_>>();

        let has_weights = !pairs.is_empty();

        let weights = quote! {
            ::goldleaf::mongodb::bson::doc!{#(#pairs),*}
        };

        // COLLATION
        let use_collation = i.case_insensitivity.is_some();
        let collation = match &i.case_insensitivity {
            None => quote! {
                ::goldleaf::mongodb::options::Collation::builder().locale("en".to_string()).build()
            },
            Some(case_insensitivity) => {
                let locale = &case_insensitivity.locale;
                let strength = case_insensitivity.strength;
                let strength = quote! {
                    match #strength {
                        1 => ::goldleaf::mongodb::options::CollationStrength::Primary,
                        2 => ::goldleaf::mongodb::options::CollationStrength::Secondary,
                        3 => ::goldleaf::mongodb::options::CollationStrength::Tertiary,
                        4 => ::goldleaf::mongodb::options::CollationStrength::Quaternary,
                        5 => ::goldleaf::mongodb::options::CollationStrength::Identical,
                        _ => panic!("Collation strength out of bounds!")
                    }
                };
                quote! {
                    ::goldleaf::mongodb::options::Collation::builder().locale(#locale.to_string()).strength(Some(#strength)).build()
                }
            },
        };

        // LANGUAGE OVERRIDES
        let language = i.pairs.iter().find_map(|p| if p.is_lang_field { Some(p.ident.clone()) } else { None }).unwrap_or("".to_string());

        // EXPIRATION
        let expiration_secs = collection.expiration_secs.unwrap_or(0);

        // PARTIAL FILTER EXPRESSIONS
        let has_pfe = i.pfe.is_some();
        let pfe: proc_macro2::TokenStream = i.pfe.clone().unwrap_or_default().parse().expect("PFE to be parseable");
        let pfe = quote! {
            ::goldleaf::mongodb::bson::doc!{#pfe}
        };

        quote! {
            ::goldleaf::mongodb::options::IndexOptions::builder()
                .name(if #index_name.is_empty() {None} else {Some(#index_name.to_string())})
                .unique(Some(#unique))
                .expire_after(if #expiration_secs > 0 {Some(::std::time::Duration::from_secs(#expiration_secs))} else {None})
                .weights(if #has_weights {Some(#weights)} else {None})
                .bits(if #use_two_d {Some(#bits)} else {None})
                .max(if #use_two_d {Some(#max)} else {None})
                .min(if #use_two_d {Some(#min)} else {None})
                .collation(if #use_collation {Some(#collation)} else {None})
                .language_override(if #language.is_empty() {None} else {Some(#language.to_string())})
                .partial_filter_expression(if #has_pfe {Some(#pfe)} else {None})
                .build()
        }
    }).collect::<Vec<_>>();

    // Concatenate strings into function call
    let calls = docs.iter().zip(opts.iter()).map(|(doc, opt)| quote! {coll.create_index(::goldleaf::mongodb::IndexModel::builder().keys(#doc).options(Some(#opt)).build()).await?;}).collect::<Vec<_>>();

    // Generate quotes
    let indices = quote! {
        impl #struct_name {
            pub async fn create_indices(db: &::goldleaf::mongodb::Database) -> Result<(), ::mongodb::error::Error> {
                let coll = <::goldleaf::mongodb::Database as ::goldleaf::AutoCollection>::auto_collection::<Self>(db);

                #(#calls)*
                Ok(())
            }
        }
    };

    // Append tokens
    let out = quote! {
        #identity

        #indices
    };

    out.into()
}

fn generate_index_pair(field: &CombinedFieldIdentityData) -> IndexPair {
    IndexPair {
        ident: match field.sub.as_ref() {
            None => field.ident.as_ref().expect("Field identifier").to_string(),
            Some(sub) => format!(
                "{}.{}",
                field.ident.as_ref().expect("Field identifier"),
                sub
            ),
        },
        index: if let Some(text_weight) = field.text_weight {
            IndexType::Text(text_weight.into())
        } else if let Some(two_d) = field.two_d.as_ref() {
            IndexType::TwoD(*two_d)
        } else {
            IndexType::Numeric(field.index.unwrap_or(1))
        },
        order_index: field.order.unwrap_or(0),
        is_lang_field: field.lang_field.is_present(),
    }
}
