use std::fs;

pub const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");

pub fn read_fixture(name: &str) -> Vec<u8> {
    fs::read(format!("{FIXTURES_DIR}/{name}")).unwrap()
}
