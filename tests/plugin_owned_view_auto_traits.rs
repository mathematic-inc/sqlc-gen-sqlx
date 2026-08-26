#![warn(rust_2018_idioms)]

use sqlc_gen_sqlx::plugin::{
    CatalogOwnedView, CodegenOwnedView, ColumnOwnedView, CompositeTypeOwnedView, EnumOwnedView,
    FileOwnedView, GenerateRequestOwnedView, GenerateResponseOwnedView, IdentifierOwnedView,
    ParameterOwnedView, QueryOwnedView, SchemaOwnedView, SettingsOwnedView, TableOwnedView,
    codegen::{ProcessOwnedView, WASMOwnedView},
};

fn require_auto_traits<T: Send + Sync + Unpin>() {}

#[test]
fn owned_views_are_send_sync_and_unpin() {
    require_auto_traits::<FileOwnedView>();
    require_auto_traits::<SettingsOwnedView>();
    require_auto_traits::<CodegenOwnedView>();
    require_auto_traits::<ProcessOwnedView>();
    require_auto_traits::<WASMOwnedView>();
    require_auto_traits::<CatalogOwnedView>();
    require_auto_traits::<SchemaOwnedView>();
    require_auto_traits::<CompositeTypeOwnedView>();
    require_auto_traits::<EnumOwnedView>();
    require_auto_traits::<TableOwnedView>();
    require_auto_traits::<IdentifierOwnedView>();
    require_auto_traits::<ColumnOwnedView>();
    require_auto_traits::<QueryOwnedView>();
    require_auto_traits::<ParameterOwnedView>();
    require_auto_traits::<GenerateRequestOwnedView>();
    require_auto_traits::<GenerateResponseOwnedView>();
}
