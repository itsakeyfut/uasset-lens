#![doc = include_str!("../README.md")]

mod db;
pub mod duplicates;
pub mod error;
mod mutations;
mod queries;
mod record;
mod schema;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
#[cfg(any(test, feature = "test-support"))]
pub use test_support::{make_record, set_schema_version};

pub use db::{AssetDb, CURRENT_SCHEMA_VERSION};
pub use error::DbError;
pub use record::{AssetFilter, AssetRecord, BlueprintRow, ScanSnapshot, TextureRow};
