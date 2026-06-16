use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AssetType {
    Blueprint,
    BlueprintInterface,
    AnimBlueprint,
    UserWidget,
    StaticMesh,
    SkeletalMesh,
    Texture2D,
    Material,
    MaterialInstance,
    MaterialFunction,
    SoundWave,
    SoundCue,
    AnimSequence,
    AnimMontage,
    LevelSequence,
    DataTable,
    DataAsset,
    World,
    ObjectRedirector,
    NiagaraSystem,
    NiagaraEmitter,
    IKRigDefinition,
    IKRetargeter,
    DialogueWave,
    Unknown(String),
}

impl fmt::Display for AssetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(s) => write!(f, "{s}"),
            _ => write!(f, "{self:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_type_should_serialize_and_deserialize_named_variant() {
        let original = AssetType::Blueprint;
        let json = serde_json::to_string(&original).unwrap();
        let roundtrip: AssetType = serde_json::from_str(&json).unwrap();
        assert_eq!(original, roundtrip);
    }

    #[test]
    fn asset_type_should_serialize_and_deserialize_all_named_variants() {
        let variants = [
            AssetType::Blueprint,
            AssetType::BlueprintInterface,
            AssetType::AnimBlueprint,
            AssetType::UserWidget,
            AssetType::StaticMesh,
            AssetType::SkeletalMesh,
            AssetType::Texture2D,
            AssetType::Material,
            AssetType::MaterialInstance,
            AssetType::MaterialFunction,
            AssetType::SoundWave,
            AssetType::SoundCue,
            AssetType::AnimSequence,
            AssetType::AnimMontage,
            AssetType::LevelSequence,
            AssetType::DataTable,
            AssetType::DataAsset,
            AssetType::World,
            AssetType::ObjectRedirector,
            AssetType::NiagaraSystem,
            AssetType::NiagaraEmitter,
            AssetType::IKRigDefinition,
            AssetType::IKRetargeter,
            AssetType::DialogueWave,
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant).unwrap();
            let roundtrip: AssetType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, &roundtrip);
        }
    }

    #[test]
    fn asset_type_should_serialize_and_deserialize_unknown_variant() {
        let original = AssetType::Unknown("CustomClass".to_owned());
        let json = serde_json::to_string(&original).unwrap();
        let roundtrip: AssetType = serde_json::from_str(&json).unwrap();
        assert_eq!(original, roundtrip);
    }

    #[test]
    fn asset_type_should_display_variant_name() {
        assert_eq!(AssetType::Blueprint.to_string(), "Blueprint");
        assert_eq!(AssetType::StaticMesh.to_string(), "StaticMesh");
        assert_eq!(AssetType::ObjectRedirector.to_string(), "ObjectRedirector");
    }

    #[test]
    fn asset_type_should_display_unknown_inner_string() {
        assert_eq!(AssetType::Unknown("Foo".to_owned()).to_string(), "Foo");
    }
}
