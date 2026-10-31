use serde::{Deserialize, Serialize};

/// Live launch status of a profile's browser. Serializes to camelCase to match
/// the TypeScript `ProfileStatus` union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileStatus {
    #[default]
    Idle,
    Launching,
    Running,
    Error,
}

impl ProfileStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            ProfileStatus::Idle => "idle",
            ProfileStatus::Launching => "launching",
            ProfileStatus::Running => "running",
            ProfileStatus::Error => "error",
        }
    }
}

/// Fingerprint operating system presented by a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileOs {
    #[default]
    Windows,
    Macos,
    Linux,
    Android,
    Ios,
}

impl ProfileOs {
    pub const fn as_str(self) -> &'static str {
        match self {
            ProfileOs::Windows => "windows",
            ProfileOs::Macos => "macos",
            ProfileOs::Linux => "linux",
            ProfileOs::Android => "android",
            ProfileOs::Ios => "ios",
        }
    }

    /// Parse the string form the database and the Tauri inputs carry. Unknown input falls back to
    /// the default rather than failing: `os` is a display/identity choice, and a profile that
    /// cannot be launched because its OS string was mistyped would be a worse outcome than one
    /// launched as Windows. The value is round-tripped through [`ProfileOs::as_str`] everywhere
    /// Veil writes it, so an unknown value means hand-edited data or an older schema.
    pub fn from_str_or_default(value: &str) -> Self {
        match value {
            "macos" => ProfileOs::Macos,
            "linux" => ProfileOs::Linux,
            "android" => ProfileOs::Android,
            "ios" => ProfileOs::Ios,
            _ => ProfileOs::Windows,
        }
    }

    /// Value for `--fingerprint-platform`, which `002`, `006` and `011` read.
    ///
    /// `None` for `ios` only, since T24-a landed the
    /// android port: `android` now maps to `Some("android")` and the engine's `990` implements
    /// every surface the claim touches. Emitting a value no patch handles is the same class of
    /// lie as emitting a switch that does not exist, and less visible, because the switch name
    /// would look supported — which is why `ios` (no precedent anywhere, no gate writable, 6.24)
    /// stays `None`. An iOS profile therefore gets no platform switch and the engine keeps
    /// reporting the host platform, which is recorded as a gap
    /// rather than hiding.
    ///
    /// `ios` is **out of the OS picker** as of T24: the profile form offers the three desktop
    /// values plus android, and the `Ios` variant survives here only so a profile stored before
    /// that keeps round-tripping (`from_str_or_default`) and launching with the honest
    /// withholding. Nothing selects `ios` again — re-adding it waits for an implementation,
    /// which nothing anywhere ports.
    pub const fn as_fingerprint_platform(self) -> Option<&'static str> {
        match self {
            ProfileOs::Windows => Some("windows"),
            ProfileOs::Macos => Some("macos"),
            ProfileOs::Linux => Some("linux"),
            ProfileOs::Android => Some("android"),
            ProfileOs::Ios => None,
        }
    }

    /// Whether this OS hands the engine a platform claim at all.
    ///
    /// The inverse of the `None` arm above, and the condition every consumer of
    /// `--fingerprint-platform` has to use. `960` derives `screen.avail*`, `colorDepth` and
    /// `orientation` from that switch, so a platform-less profile can declare no display block:
    /// `to_engine_args` emits none of it and `validate` refuses one. Both sides read the answer
    /// from here rather than repeating the match, because a second copy of this rule is exactly
    /// the shape of defect this project keeps removing.
    pub const fn has_fingerprint_platform(self) -> bool {
        self.as_fingerprint_platform().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_default_is_idle() {
        assert_eq!(ProfileStatus::default(), ProfileStatus::Idle);
        assert_eq!(ProfileStatus::Running.as_str(), "running");
    }

    #[test]
    fn os_serializes_camel_case() {
        assert_eq!(
            serde_json::to_string(&ProfileOs::Macos).unwrap(),
            r#""macos""#
        );
    }

    /// T24: every variant keeps parsing and serialising, so a profile stored under any picker
    /// generation opens instead of failing to load. What changed is the *claim*: android re-joined
    /// the implemented set when 6.26's port landed, and ios still promises the engine nothing —
    /// no precedent anywhere, no path to one (6.24).
    #[test]
    fn the_mobile_os_values_round_trip_and_claim_what_the_engine_implements() {
        for os in [ProfileOs::Android, ProfileOs::Ios] {
            assert_eq!(ProfileOs::from_str_or_default(os.as_str()), os);
        }
        assert_eq!(
            ProfileOs::Android.as_fingerprint_platform(),
            Some("android")
        );
        assert!(ProfileOs::Android.has_fingerprint_platform());
        assert_eq!(ProfileOs::Ios.as_fingerprint_platform(), None);
        assert!(!ProfileOs::Ios.has_fingerprint_platform());
        for os in [ProfileOs::Windows, ProfileOs::Macos, ProfileOs::Linux] {
            assert!(os.has_fingerprint_platform());
        }
    }
}
