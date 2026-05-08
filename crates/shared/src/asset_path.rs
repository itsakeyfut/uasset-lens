use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AssetPath(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssetPathError {
    #[error("asset path must not be empty")]
    Empty,
    #[error("asset path must start with '/'")]
    MissingLeadingSlash,
    #[error("asset path must not contain a file extension (found '.' after last '/')")]
    InvalidCharacter,
    #[error("file path is not under the content root")]
    NotUnderContentRoot,
}

impl AssetPath {
    pub fn new(s: &str) -> Result<Self, AssetPathError> {
        if s.is_empty() {
            return Err(AssetPathError::Empty);
        }
        if !s.starts_with('/') {
            return Err(AssetPathError::MissingLeadingSlash);
        }
        let last_segment = s.rsplit('/').next().unwrap_or("");
        if last_segment.contains('.') {
            return Err(AssetPathError::InvalidCharacter);
        }
        Ok(Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_fs_path(content_root: &Path, file_path: &Path) -> Result<Self, AssetPathError> {
        let relative = file_path
            .strip_prefix(content_root)
            .map_err(|_| AssetPathError::NotUnderContentRoot)?;

        let without_ext = relative.with_extension("");

        let game_path = format!(
            "/Game/{}",
            without_ext
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        );

        Ok(Self(game_path))
    }

    pub fn package_name(&self) -> &str {
        match self.0.rfind('/') {
            None => &self.0,
            Some(slash_pos) => {
                let last = &self.0[slash_pos + 1..];
                match last.find('.') {
                    None => &self.0,
                    Some(dot_pos) => &self.0[..slash_pos + 1 + dot_pos],
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn new_should_return_error_for_empty_string() {
        assert_eq!(AssetPath::new(""), Err(AssetPathError::Empty));
    }

    #[test]
    fn new_should_return_error_when_missing_leading_slash() {
        assert_eq!(
            AssetPath::new("Game/Characters/BP_Player"),
            Err(AssetPathError::MissingLeadingSlash)
        );
    }

    #[test]
    fn new_should_return_error_when_path_contains_extension() {
        assert_eq!(
            AssetPath::new("/Game/Characters/BP_Player.uasset"),
            Err(AssetPathError::InvalidCharacter)
        );
    }

    #[test]
    fn new_should_succeed_for_valid_path() {
        let path = AssetPath::new("/Game/Characters/BP_Player").unwrap();
        assert_eq!(path.as_str(), "/Game/Characters/BP_Player");
    }

    #[test]
    fn from_fs_path_should_convert_uasset_path_to_game_path() {
        let content_root = PathBuf::from("/Project/Content");
        let file_path = PathBuf::from("/Project/Content/Characters/BP_Player.uasset");
        let path = AssetPath::from_fs_path(&content_root, &file_path).unwrap();
        assert_eq!(path.as_str(), "/Game/Characters/BP_Player");
    }

    #[test]
    fn from_fs_path_should_return_error_when_not_under_content_root() {
        let content_root = PathBuf::from("/Project/Content");
        let file_path = PathBuf::from("/Other/Characters/BP_Player.uasset");
        assert_eq!(
            AssetPath::from_fs_path(&content_root, &file_path),
            Err(AssetPathError::NotUnderContentRoot)
        );
    }

    #[test]
    fn package_name_should_strip_object_suffix_when_present() {
        let path = AssetPath::new("/Game/Characters/BP_Player").unwrap();
        let suffixed = AssetPath(format!("{}.BP_Player", path.as_str()));
        assert_eq!(suffixed.package_name(), "/Game/Characters/BP_Player");
    }

    #[test]
    fn package_name_should_return_full_path_when_no_suffix() {
        let path = AssetPath::new("/Game/Characters/BP_Player").unwrap();
        assert_eq!(path.package_name(), "/Game/Characters/BP_Player");
    }
}
