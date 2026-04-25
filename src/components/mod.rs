pub trait Component {
    fn name() -> &'static str;
    fn from_yaml(value: &serde_yaml::Value) -> anyhow::Result<Self>
    where
        Self: Sized;
}
