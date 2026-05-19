#[derive(Debug, Clone, Copy)]
pub struct FPackageVersion {
    pub legacy_version: i32,
    pub file_version_ue5: u32,
}

impl FPackageVersion {
    pub fn is_ue5(&self) -> bool {
        self.legacy_version == -8 && self.file_version_ue5 > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_ue5_should_return_true_for_ue5_package() {
        let v = FPackageVersion {
            legacy_version: -8,
            file_version_ue5: 1005,
        };
        assert!(v.is_ue5());
    }

    #[test]
    fn is_ue5_should_return_false_when_legacy_version_is_not_minus_8() {
        let v = FPackageVersion {
            legacy_version: -7,
            file_version_ue5: 1005,
        };
        assert!(!v.is_ue5());
    }

    #[test]
    fn is_ue5_should_return_false_when_file_version_ue5_is_zero() {
        let v = FPackageVersion {
            legacy_version: -8,
            file_version_ue5: 0,
        };
        assert!(!v.is_ue5());
    }
}
