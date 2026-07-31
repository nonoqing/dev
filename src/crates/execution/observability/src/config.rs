use crate::TelemetryLevel;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

pub const TELEMETRY_USER_CONFIG_VERSION: u16 = 1;

/// User-controlled telemetry consent. Product endpoint, credentials, sampling,
/// and capacity settings deliberately do not appear in this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryUserConfigV1 {
    pub version: u16,
    pub level: TelemetryLevel,
}

impl TelemetryUserConfigV1 {
    pub const fn new(level: TelemetryLevel) -> Self {
        Self {
            version: TELEMETRY_USER_CONFIG_VERSION,
            level,
        }
    }
}

impl Default for TelemetryUserConfigV1 {
    fn default() -> Self {
        Self::new(TelemetryLevel::Off)
    }
}

/// Version-tolerant persisted user configuration.
///
/// Unknown newer objects are retained byte-for-byte at the JSON value level,
/// but execute as `off` until this client understands their semantics.
#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryUserConfig {
    V1(TelemetryUserConfigV1),
    Unknown(serde_json::Value),
}

impl TelemetryUserConfig {
    pub const fn new(level: TelemetryLevel) -> Self {
        Self::V1(TelemetryUserConfigV1::new(level))
    }

    pub const fn effective_level(&self) -> TelemetryLevel {
        match self {
            Self::V1(config) => config.level,
            Self::Unknown(_) => TelemetryLevel::Off,
        }
    }

    pub const fn v1(&self) -> Option<TelemetryUserConfigV1> {
        match self {
            Self::V1(config) => Some(*config),
            Self::Unknown(_) => None,
        }
    }

    pub fn set_level(&mut self, level: TelemetryLevel) {
        *self = Self::V1(TelemetryUserConfigV1::new(level));
    }
}

impl Default for TelemetryUserConfig {
    fn default() -> Self {
        Self::V1(TelemetryUserConfigV1::default())
    }
}

impl Serialize for TelemetryUserConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::V1(config) => config.serialize(serializer),
            Self::Unknown(raw) => raw.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for TelemetryUserConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = serde_json::Value::deserialize(deserializer)?;
        if let Some(enabled) = raw.as_bool() {
            return Ok(Self::V1(TelemetryUserConfigV1::new(if enabled {
                TelemetryLevel::Basic
            } else {
                TelemetryLevel::Off
            })));
        }

        let version = raw
            .as_object()
            .ok_or_else(|| D::Error::custom("telemetry config must be a boolean or object"))?
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::from(TELEMETRY_USER_CONFIG_VERSION));
        if version != u64::from(TELEMETRY_USER_CONFIG_VERSION) {
            return Ok(Self::Unknown(raw));
        }
        let parsed = TelemetryUserConfigV1::deserialize(raw).map_err(D::Error::custom)?;
        Ok(Self::V1(parsed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_boolean_migrates_and_serializes_as_v1() {
        let enabled: TelemetryUserConfig = serde_json::from_str("true").unwrap();
        assert_eq!(enabled.effective_level(), TelemetryLevel::Basic);
        assert_eq!(
            serde_json::to_value(enabled).unwrap(),
            serde_json::json!({"version": 1, "level": "basic"})
        );

        let disabled: TelemetryUserConfig = serde_json::from_str("false").unwrap();
        assert_eq!(disabled.effective_level(), TelemetryLevel::Off);
    }

    #[test]
    fn unknown_version_is_preserved_but_executes_as_off() {
        let raw = serde_json::json!({
            "version": 99,
            "level": "diagnostic",
            "future_setting": {"enabled": true}
        });
        let config: TelemetryUserConfig = serde_json::from_value(raw.clone()).unwrap();

        assert_eq!(config.effective_level(), TelemetryLevel::Off);
        assert_eq!(serde_json::to_value(config).unwrap(), raw);
    }
}
