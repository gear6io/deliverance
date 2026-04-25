use std::{fmt, str::FromStr};

/// Validated component type string.
/// Rules (mirrors OTel's `component.Type`): ASCII alpha start, alphanumeric + `_` only, max 63 chars.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentType(String);

impl ComponentType {
    pub fn new(s: impl Into<String>) -> anyhow::Result<Self> {
        let s = s.into();
        if s.is_empty() || !s.starts_with(|c: char| c.is_ascii_alphabetic()) {
            anyhow::bail!("component type {:?} must start with an ASCII letter", s);
        }
        if s.len() > 63 {
            anyhow::bail!("component type {:?} exceeds 63 characters", s);
        }
        if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            anyhow::bail!(
                "component type {:?} contains invalid characters (only [a-zA-Z0-9_] allowed)",
                s
            );
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ComponentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Component identifier in the form `{type}` or `{type}/{name}`.
///
/// Mirrors OTel's `component.ID`. The type selects the registered factory; the name
/// disambiguates multiple instances of the same type within a config file.
///
/// Examples: `"http"`, `"http/ingest_a"`, `"batch/high_priority"`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentID {
    component_type: ComponentType,
    name: Option<String>,
}

impl ComponentID {
    pub fn new(component_type: ComponentType) -> Self {
        Self {
            component_type,
            name: None,
        }
    }

    pub fn with_name(component_type: ComponentType, name: impl Into<String>) -> Self {
        Self {
            component_type,
            name: Some(name.into()),
        }
    }

    pub fn component_type(&self) -> &ComponentType {
        &self.component_type
    }

    pub fn type_str(&self) -> &str {
        self.component_type.as_str()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

impl FromStr for ComponentID {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.split_once('/') {
            Some((type_str, name)) => {
                let name = name.trim();
                if name.is_empty() {
                    anyhow::bail!("component id {:?}: name after '/' must not be empty", s);
                }
                Ok(Self::with_name(ComponentType::new(type_str.trim())?, name))
            }
            None => Ok(Self::new(ComponentType::new(s.trim())?)),
        }
    }
}

impl fmt::Display for ComponentID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Some(n) => write!(f, "{}/{}", self.component_type, n),
            None => write!(f, "{}", self.component_type),
        }
    }
}

/// Passed to every factory `create()` call. Mirrors OTel's `component.Settings`.
/// Carries the resolved ID so the component knows its own identity (useful for logging).
#[derive(Debug, Clone)]
pub struct Settings {
    pub id: ComponentID,
}

impl Settings {
    pub fn new(id: ComponentID) -> Self {
        Self { id }
    }
}

/// Marker trait for all pipeline components.
/// `component_type()` must match the string used when registering the factory.
pub trait Component {
    fn component_type() -> &'static str
    where
        Self: Sized;
}
