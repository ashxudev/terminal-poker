//! Versioned, bounded local presentation and Quick Practice preferences.

use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::ui::platform::ThemeMode;

const PROFILE_FILE: &str = "profile.json";
const PROFILE_SCHEMA_VERSION: u32 = 2;
const MAX_PROFILE_BYTES: u64 = 16 * 1024;
const MAX_DISPLAY_NAME_CHARS: usize = 24;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalProfile {
    pub schema_version: u32,
    pub display_name: String,
    pub theme: ProfileTheme,
    pub reduced_motion: bool,
    pub quick_starting_stack: u32,
    #[serde(default)]
    pub server_address: Option<String>,
}

impl Default for LocalProfile {
    fn default() -> Self {
        Self {
            schema_version: PROFILE_SCHEMA_VERSION,
            display_name: "Player".to_string(),
            theme: ProfileTheme::Ash,
            reduced_motion: false,
            quick_starting_stack: 100,
            server_address: None,
        }
    }
}

impl LocalProfile {
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.schema_version != PROFILE_SCHEMA_VERSION {
            return Err(ProfileError::UnsupportedVersion(self.schema_version));
        }
        if let Some(address) = &self.server_address {
            crate::game_invite::game_server_address(address)
                .map_err(|message| ProfileError::Invalid(message.into()))?;
        }
        let name_len = self.display_name.chars().count();
        if name_len == 0
            || name_len > MAX_DISPLAY_NAME_CHARS
            || self.display_name.chars().any(char::is_control)
        {
            return Err(ProfileError::Invalid(
                "display name must contain 1-24 visible characters".to_string(),
            ));
        }
        if !(20..=1_000_000).contains(&self.quick_starting_stack) {
            return Err(ProfileError::Invalid(
                "Quick Practice starting stack must be between 20 and 1,000,000".to_string(),
            ));
        }
        Ok(())
    }

    pub const fn theme_mode(&self) -> ThemeMode {
        // Legacy fields remain readable so existing profiles keep player data.
        // Appearance is unified; saved theme choices no longer select a UI.
        ThemeMode::Ash
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileTheme {
    Ash,
    HighContrast,
}

#[derive(Debug, Clone)]
pub struct ProfileStore {
    root: PathBuf,
}

impl ProfileStore {
    pub fn platform_default() -> Result<Self, ProfileError> {
        let root = dirs::config_dir()
            .ok_or(ProfileError::NoPlatformDirectory)?
            .join("sneakyblinders");
        Ok(Self { root })
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self) -> PathBuf {
        self.root.join(PROFILE_FILE)
    }

    pub fn load(&self) -> Result<Option<LocalProfile>, ProfileError> {
        let path = self.path();
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ProfileError::Io(error)),
        };
        let length = file.metadata()?.len();
        if length > MAX_PROFILE_BYTES {
            return Err(ProfileError::TooLarge(length));
        }
        let mut bytes = Vec::with_capacity(length as usize);
        file.take(MAX_PROFILE_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_PROFILE_BYTES {
            return Err(ProfileError::TooLarge(bytes.len() as u64));
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| ProfileError::Malformed(error.to_string()))?;
        let version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let profile = match version {
            0 => migrate_v0(value)?,
            1 | 2 => {
                let mut profile: LocalProfile = serde_json::from_value(value)
                    .map_err(|error| ProfileError::Malformed(error.to_string()))?;
                profile.schema_version = PROFILE_SCHEMA_VERSION;
                profile
            }
            other => return Err(ProfileError::UnsupportedVersion(other as u32)),
        };
        profile.validate()?;
        Ok(Some(profile))
    }

    pub fn save(&self, profile: &LocalProfile) -> Result<(), ProfileError> {
        profile.validate()?;
        fs::create_dir_all(&self.root)?;
        let bytes = serde_json::to_vec_pretty(profile)
            .map_err(|error| ProfileError::Malformed(error.to_string()))?;
        if bytes.len() as u64 > MAX_PROFILE_BYTES {
            return Err(ProfileError::TooLarge(bytes.len() as u64));
        }

        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = self.root.join(format!(
            ".{PROFILE_FILE}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        let write_result = (|| -> Result<(), ProfileError> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            drop(file);
            crate::table_registry::atomic_replace(&temporary, &self.path())?;
            if let Ok(directory) = File::open(&self.root) {
                let _ = directory.sync_all();
            }
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        write_result
    }
}

#[derive(Debug)]
pub enum ProfileError {
    Io(std::io::Error),
    NoPlatformDirectory,
    TooLarge(u64),
    UnsupportedVersion(u32),
    Malformed(String),
    Invalid(String),
}

impl Display for ProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "profile I/O failed: {error}"),
            Self::NoPlatformDirectory => {
                formatter.write_str("the operating system has no user configuration directory")
            }
            Self::TooLarge(bytes) => write!(
                formatter,
                "profile is {bytes} bytes; the safe limit is {MAX_PROFILE_BYTES}"
            ),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "profile schema {version} is not supported")
            }
            Self::Malformed(message) => write!(formatter, "profile is malformed: {message}"),
            Self::Invalid(message) => write!(formatter, "profile is invalid: {message}"),
        }
    }
}

impl std::error::Error for ProfileError {}

impl From<std::io::Error> for ProfileError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileV0 {
    #[serde(default)]
    schema_version: u32,
    display_name: String,
    #[serde(default)]
    high_contrast: bool,
    #[serde(default)]
    reduced_motion: bool,
    #[serde(default = "default_starting_stack")]
    quick_starting_stack: u32,
}

fn migrate_v0(value: serde_json::Value) -> Result<LocalProfile, ProfileError> {
    let legacy: ProfileV0 = serde_json::from_value(value)
        .map_err(|error| ProfileError::Malformed(error.to_string()))?;
    debug_assert_eq!(legacy.schema_version, 0);
    Ok(LocalProfile {
        schema_version: PROFILE_SCHEMA_VERSION,
        display_name: legacy.display_name,
        theme: if legacy.high_contrast {
            ProfileTheme::HighContrast
        } else {
            ProfileTheme::Ash
        },
        reduced_motion: legacy.reduced_motion,
        quick_starting_stack: legacy.quick_starting_stack,
        server_address: None,
    })
}

const fn default_starting_stack() -> u32 {
    100
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_store(label: &str) -> ProfileStore {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        ProfileStore::at(std::env::temp_dir().join(format!(
            "sneakyblinders-profile-{label}-{}-{stamp}",
            std::process::id()
        )))
    }

    #[test]
    fn v1_migrates_and_v2_remembers_only_the_server() {
        let store = test_store("server");
        fs::create_dir_all(store.root()).unwrap();
        let source = br#"{"schema_version":1,"display_name":"Ada","theme":"ash","reduced_motion":false,"quick_starting_stack":100}"#;
        fs::write(store.path(), source).unwrap();
        let mut profile = store.load().unwrap().unwrap();
        assert_eq!(profile.schema_version, 2);
        assert_eq!(profile.server_address, None);
        assert_eq!(fs::read(store.path()).unwrap(), source);
        profile.server_address = Some("127.0.0.1:7777".into());
        store.save(&profile).unwrap();
        assert_eq!(store.load().unwrap(), Some(profile));
        let stored = fs::read_to_string(store.path()).unwrap();
        assert!(!stored.contains("password"));
        assert!(!stored.contains("join_code"));
        fs::remove_dir_all(store.root()).unwrap();
    }

    #[test]
    fn profile_round_trip_is_versioned_atomic_and_platform_independent() {
        let store = test_store("round-trip");
        assert_eq!(store.load().unwrap(), None);
        let profile = LocalProfile {
            display_name: "Ada".to_string(),
            theme: ProfileTheme::HighContrast,
            reduced_motion: true,
            quick_starting_stack: 250,
            ..LocalProfile::default()
        };
        store.save(&profile).unwrap();
        assert_eq!(store.load().unwrap().unwrap().theme_mode(), ThemeMode::Ash);
        assert_eq!(store.load().unwrap(), Some(profile));
        assert_eq!(
            fs::read_dir(store.root()).unwrap().count(),
            1,
            "temporary publication files must not remain"
        );
        fs::remove_dir_all(store.root()).unwrap();
    }

    #[test]
    fn legacy_profile_migrates_in_memory_without_overwriting_source() {
        let store = test_store("migration");
        fs::create_dir_all(store.root()).unwrap();
        let source = br#"{"schema_version":0,"display_name":"Grace","high_contrast":true,"reduced_motion":true,"quick_starting_stack":500}"#;
        fs::write(store.path(), source).unwrap();
        let profile = store.load().unwrap().unwrap();
        assert_eq!(profile.schema_version, 2);
        assert_eq!(profile.theme, ProfileTheme::HighContrast);
        assert_eq!(fs::read(store.path()).unwrap(), source);
        fs::remove_dir_all(store.root()).unwrap();
    }

    #[test]
    fn corrupt_newer_and_oversized_profiles_fail_without_overwrite() {
        for (label, bytes) in [
            ("corrupt", b"not-json".to_vec()),
            (
                "newer",
                br#"{"schema_version":99,"display_name":"Future"}"#.to_vec(),
            ),
            ("oversized", vec![b'x'; MAX_PROFILE_BYTES as usize + 1]),
        ] {
            let store = test_store(label);
            fs::create_dir_all(store.root()).unwrap();
            fs::write(store.path(), &bytes).unwrap();
            assert!(store.load().is_err());
            assert_eq!(fs::read(store.path()).unwrap(), bytes);
            fs::remove_dir_all(store.root()).unwrap();
        }
    }

    #[test]
    fn profile_rejects_controls_secrets_and_unbounded_values_by_schema() {
        let profile = LocalProfile {
            display_name: "bad\nname".to_string(),
            ..LocalProfile::default()
        };
        assert!(profile.validate().is_err());

        let serialized = serde_json::to_string(&LocalProfile::default()).unwrap();
        for forbidden in ["credential", "token", "hole_cards", "deal_plan", "history"] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[test]
    fn concurrent_instances_publish_whole_profiles_without_temp_leaks() {
        let store = test_store("concurrent");
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|index| {
                let store = store.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let profile = LocalProfile {
                        display_name: format!("Player {index}"),
                        quick_starting_stack: 100 + index * 10,
                        ..LocalProfile::default()
                    };
                    barrier.wait();
                    store.save(&profile).unwrap();
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap();
        }

        let published = store.load().unwrap().unwrap();
        assert!(published.display_name.starts_with("Player "));
        assert!((100..=170).contains(&published.quick_starting_stack));
        assert_eq!(
            fs::read_dir(store.root()).unwrap().count(),
            1,
            "concurrent writers must leave one valid profile and no temporary files"
        );
        fs::remove_dir_all(store.root()).unwrap();
    }
}
