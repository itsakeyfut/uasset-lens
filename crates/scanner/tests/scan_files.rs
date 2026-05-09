use scanner::{ScanError, scan_files};
use shared::AssetType;
use std::collections::HashMap;
use std::path::Path;

const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");

#[test]
fn scan_files_should_detect_correct_asset_types_for_all_valid_fixtures() {
    let valid = Path::new(FIXTURES_DIR).join("valid");
    let files = vec![
        valid.join("BP_Simple.uasset"),
        valid.join("M_Basic.uasset"),
        valid.join("Redirect.uasset"),
        valid.join("SM_Cube.uasset"),
        valid.join("T_Rock.uasset"),
        valid.join("L_TestMap.umap"),
    ];

    let result = scan_files(&files, &valid);

    assert_eq!(
        result.skipped.len(),
        0,
        "unexpected skips: {:?}",
        result
            .skipped
            .iter()
            .map(|s| &s.file_path)
            .collect::<Vec<_>>()
    );
    assert_eq!(result.assets.len(), 6);

    let by_path: HashMap<_, _> = result
        .assets
        .iter()
        .map(|a| (a.asset_path.as_str().to_owned(), &a.asset_type))
        .collect();

    assert_eq!(by_path["/Game/BP_Simple"], &AssetType::Blueprint);
    assert_eq!(by_path["/Game/M_Basic"], &AssetType::Material);
    assert_eq!(by_path["/Game/Redirect"], &AssetType::ObjectRedirector);
    assert_eq!(by_path["/Game/SM_Cube"], &AssetType::StaticMesh);
    assert_eq!(by_path["/Game/T_Rock"], &AssetType::Texture2D);
    assert_eq!(by_path["/Game/L_TestMap"], &AssetType::World);
}

#[test]
fn scan_files_should_add_bad_magic_to_skipped_with_invalid_magic_error() {
    let valid = Path::new(FIXTURES_DIR).join("valid");
    let invalid = Path::new(FIXTURES_DIR).join("invalid");
    let files = vec![invalid.join("bad_magic.bin")];

    let result = scan_files(&files, &valid);

    assert_eq!(result.assets.len(), 0);
    assert_eq!(result.skipped.len(), 1);
    assert!(
        matches!(result.skipped[0].reason, ScanError::InvalidMagic(_)),
        "expected InvalidMagic, got {:?}",
        result.skipped[0].reason
    );
}

#[test]
fn scan_files_should_add_truncated_to_skipped_with_unexpected_eof_error() {
    let valid = Path::new(FIXTURES_DIR).join("valid");
    let invalid = Path::new(FIXTURES_DIR).join("invalid");
    let files = vec![invalid.join("truncated.bin")];

    let result = scan_files(&files, &valid);

    assert_eq!(result.assets.len(), 0);
    assert_eq!(result.skipped.len(), 1);
    assert!(
        matches!(result.skipped[0].reason, ScanError::UnexpectedEof),
        "expected UnexpectedEof, got {:?}",
        result.skipped[0].reason
    );
}
