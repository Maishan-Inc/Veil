//! Per-profile fingerprint **configuration** and its mapping to engine launch flags.
//!
//! Veil does NOT implement fingerprinting — the engine does. This module only describes
//! the knobs a user can set and turns them into command-line flags the pluggable engine
//! reads. `to_engine_args` is therefore a pure string mapper with no browser logic, mirroring
//! how `--proxy-server` is built today. Keep it that way: never reimplement fingerprint logic
//! in this repo.
//!
//! Which switches may be emitted depends on [`FlagDialect`], declared by the installed engine
//! in `crates/runtime/engine.toml`. Chromium ignores unknown switches silently, so emitting a
//! flag the engine does not implement would produce a profile that reports itself as spoofed
//! while leaking everything.

use serde::{Deserialize, Serialize};

use crate::{ErrorCode, ProfileOs};

/// Which launch-switch vocabulary the installed engine actually implements.
///
/// Declared as data in `crates/runtime/engine.toml` (`[capabilities] flag_dialect`) and
/// re-exported by `veil_runtime::manifest`; it lives here because [`to_engine_args`] takes it
/// and `veil_core` depends on no internal crate.
///
/// A dialect never emits a switch it does not implement: adding an entry to
/// [`FlagDialect::VeilPatched`] requires the corresponding patch to exist in `Veil-Chromium`
/// first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlagDialect {
    /// Clean Chromium / ungoogled-chromium: user agent, window size, locale, languages, proxy
    /// and WebRTC policy only. The default, because absent information means "assume no
    /// capability".
    #[default]
    StockChromium,
    /// An engine carrying Veil's fingerprint patch set: the stock set plus `--fingerprint=<seed>`,
    /// `--fingerprint-platform=`, `--timezone=`, `--fingerprint-hardware-concurrency=` and
    /// `--disable-spoofing=`. That is the whole Tier 1 vocabulary — every other spoofed surface is
    /// derived from the seed and has no switch.
    VeilPatched,
}

/// Everything a fingerprint contributes to a launch: switches plus the environment variables
/// that carry values Chromium has no switch for.
///
/// The two channels are kept apart on purpose — env must not be smuggled through argv, and a
/// value belongs to exactly one of them per dialect (timezone is `TZ` on stock and
/// `--timezone=` on patched engines, never both).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchOptions {
    pub args: Vec<String>,
    /// Environment variables the bridge must set on the child process (e.g. `TZ`).
    pub env: Vec<(String, String)>,
}

/// WebRTC IP-handling policy presented by the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebrtcPolicy {
    /// Leave the engine default untouched.
    #[default]
    Default,
    /// Force all traffic through the proxy, hiding the local IP.
    ProxyOnly,
    /// The most restrictive setting Chromium has. **Not** a true off switch: no Chromium switch
    /// disables WebRTC, so this is `ProxyOnly`'s value under a stronger label — see
    /// `as_ip_handling_policy`.
    Disabled,
}

impl WebrtcPolicy {
    /// Value for `--webrtc-ip-handling-policy`. That is the switch the **`chrome` binary** reads:
    /// `chrome/browser/prefs/chrome_command_line_pref_store.cc:54` maps
    /// `switches::kWebRtcIPHandlingPolicy` (`chrome/common/chrome_switches.cc:726`) onto
    /// `prefs::kWebRTCIPHandlingPolicy`, and nothing else in `chrome/` touches that pref from the
    /// command line. Accepted values are `default`, `default_public_and_private_interfaces`,
    /// `default_public_interface_only` and `disable_non_proxied_udp`.
    ///
    /// `--force-webrtc-ip-handling-policy`, which this emitted until Stage 6's T03, is a *real*
    /// Chromium switch — `content/public/common/content_switches.cc:859` — and that is exactly why the
    /// mistake survived three stages. It is read only by `content/shell` and `headless/lib`, never by
    /// `chrome/`, so Veil launching `chrome.exe` passed a flag with no reader and the WebRTC field did
    /// nothing at all. Measured, both spellings, back to back.
    ///
    /// `None` means "leave the engine default alone", expressed by omitting the switch rather than
    /// by passing `default`. On the shipping engine that default is **not** upstream's:
    /// ungoogled-chromium's `default-webrtc-ip-handling-policy.patch` registers the pref as
    /// `disable_non_proxied_udp`, so `Default` gathers no ICE candidate at all here. Measured:
    /// stock Chrome 151 on the same host gathers one, this engine gathers zero.
    ///
    /// `Disabled` returns the same value as `ProxyOnly`. It returned `default_public_interface_only`
    /// before T03, which still gathers a server-reflexive candidate through the default route
    /// (measured) — so once the switch name started reaching the engine, keeping that value would
    /// have made the strictest UI choice the leakiest of the three. Two labels for one behaviour is
    /// a UI honesty problem; a leak sold as "Disabled" is a security one, and
    /// the project's priority order picks between them.
    const fn as_ip_handling_policy(self) -> Option<&'static str> {
        match self {
            WebrtcPolicy::Default => None,
            WebrtcPolicy::ProxyOnly | WebrtcPolicy::Disabled => Some("disable_non_proxied_udp"),
        }
    }
}

/// Noise strategy for canvas / audio readouts.
///
/// The level is **not** expressible to a Tier 1 engine: there is no `--canvas-noise` /
/// `--audio-noise` switch, and the amount is derived from `--fingerprint=<seed>`. `Low` and
/// `High` therefore produce identical engine behaviour; only `Off` reaches the engine, as a
/// `--disable-spoofing=` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoiseMode {
    #[default]
    Off,
    Low,
    High,
}

/// A profile's fingerprint configuration. Stored as a single JSON column on `profiles` and
/// serialized camelCase to match the TypeScript `Fingerprint` contract in `@veil/shared`.
/// Every field is optional in spirit: empty / zero means "let the engine decide", so a fresh
/// `Default` produces no flags at all and the profile launches with the engine's stock values.
///
/// `seed != 0` emits `--fingerprint=<seed>` on an engine whose patch set derives its entire
/// identity from a single integer rather than per-property flags. When set, the seed is the
/// authoritative identity; all other fields are advisory / preserved for UI display.
///
/// The configuration is stored in full regardless of the installed engine's [`FlagDialect`], so
/// switching to a patched engine does not require re-entering it. Which fields actually reach the
/// engine is decided by [`to_engine_args`], not by what is stored here.
///
/// Under [`FlagDialect::VeilPatched`] the fields fall into three states, and they are distinct
/// enough that the UI has a separate label for each:
///
/// - **settable** — the stored value reaches the engine: `seed`, `timezone`,
///   `hardware_concurrency`, `panel_width` / `panel_height` / `device_pixel_ratio`,
///   `device_memory` and `max_touch_points` (both since Stage 6 phase 4), plus the
///   stock-dialect `user_agent`, `locale`, `languages`, `webrtc_policy`.
/// - **spoofed, not configurable** — the surface *is* spoofed, from the seed, and the stored value
///   is not used as given: `canvas_noise` / `audio_noise` (only `Off` is expressible; the level is
///   not).
/// - **unsupported** — no switch exists in either dialect, so the value reaches nothing:
///   `webgl_vendor`, `webgl_renderer`, `fonts`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Fingerprint {
    /// Engine-specific seed that deterministically derives the full fingerprint identity.
    /// `0` = let the engine randomize on each launch. Non-zero emits `--fingerprint=<seed>`
    /// under [`FlagDialect::VeilPatched`]; stock Chromium has no such switch.
    pub seed: u64,
    pub user_agent: String,
    /// Window size, not `screen.*`: emitted as `--window-size` under both dialects. The field name
    /// is the stored JSON key and predates `960`, which is the only reason it is not `window_*`:
    /// renaming it would free `screenWidth` for the panel below, and a stored `screenWidth: 1280`
    /// read back under the new meaning would silently turn one profile's *window* into another
    /// profile's *panel*. The label the user sees says "window" as of T06.
    pub screen_width: u32,
    pub screen_height: u32,
    /// The display panel the profile declares, in DIPs — the unit `screen.width` / `screen.height`
    /// report. Read by `960` off `--fingerprint-screen-{width,height}`, which `000` has declared
    /// since it was ported and nothing read until then. `avail*`, `colorDepth`, `orientation` and
    /// `isExtended` are **derived** from this pair and from `os` inside the engine and have no field
    /// here: a second source for the work area is a way to fail assertion 6 R1 by hand.
    pub panel_width: u32,
    pub panel_height: u32,
    /// `window.devicePixelRatio`, `(resolution:)` and `ScreenDetailed.devicePixelRatio`, via
    /// `--fingerprint-device-scale-factor`. `0.0` means "leave the host's".
    ///
    /// A float is why [`Fingerprint`] no longer derives `Eq`. The alternative — an integer in
    /// hundredths — would put a second representation of one number between the UI and the switch,
    /// and 1.25 / 1.5 / 1.75 are ordinary Windows scale factors that have to survive the round trip.
    pub device_pixel_ratio: f64,
    pub timezone: String,
    pub locale: String,
    pub languages: Vec<String>,
    /// Display-only. Tier 1 picks the unmasked pair from a built-in table by
    /// `fingerprint % GetGpuCount()`; there is no `--webgl-vendor` / `--webgl-renderer`.
    pub webgl_vendor: String,
    pub webgl_renderer: String,
    /// Only `Off` reaches the engine, as a `--disable-spoofing=` key — see [`NoiseMode`].
    pub canvas_noise: NoiseMode,
    pub audio_noise: NoiseMode,
    /// Display-only. `006` hides a seed-derived subset of the host's families; it cannot be
    /// handed a list.
    pub fonts: Vec<String>,
    pub hardware_concurrency: u8,
    /// `navigator.deviceMemory`, in GB, via `--fingerprint-device-memory` (`905`, Stage 6 T15).
    /// `0` means "leave the host's", which is what the engine falls back to — the same two rungs
    /// `hardware_concurrency` above has had since `005`.
    ///
    /// **Only five values are expressible**, and that is Chromium's own arithmetic rather than a Veil
    /// restriction: `ApproximatedDeviceMemory` rounds physical memory to the nearer power of two and
    /// then clamps to `[2, 32]` on desktop, so 2, 4, 8, 16 and 32 are the only numbers a desktop
    /// Chrome 151 can report. `validate` enforces the set.
    /// Until T15 this field reached nothing at all — `005` returned 8 unconditionally (5.1e).
    pub device_memory: u8,
    /// `navigator.maxTouchPoints`, and with it `(pointer:)`, `(any-pointer:)`, `(hover:)` and
    /// `(any-hover:)` — five surfaces `961` answers from this one number, because it overrides the
    /// browser-side `WebPreferences` block all five are read from rather than the getter.
    ///
    /// `None` is "leave the host's", not zero: a Mac reports **0** touch points structurally, so 0 is
    /// a claim a profile has to be able to make and cannot double as the unset sentinel. That is the
    /// one place this struct needs an `Option` — `device_pixel_ratio: 0.0` and `device_memory: 0` can
    /// use a sentinel because neither zero is a machine.
    ///
    /// `validate` requires 0 on a macOS profile and bounds the rest at 20.
    pub max_touch_points: Option<u8>,
    /// The persona's declared position, as `"lat,lng"` — the exact value `970` reads off
    /// `--fingerprint-location` and `navigator.geolocation` then reports. One representation on
    /// purpose: the string is the switch's own format, and a second structured copy would be one
    /// more pair of values that can drift apart before launch.
    ///
    /// This is the **only** geography field a profile carries, and that is a design constraint
    /// rather than an omission: Veil is offline and holds no geo-IP table, so nothing here can be
    /// derived from the proxy exit the way `randomize_for_proxy` derives the timezone. The user
    /// points at the place; `validate` refuses a malformed or out-of-range value, refuses
    /// coordinates without the timezone that locates them in time, and the save path refuses a
    /// timezone that contradicts a tested proxy's answer — see `validate` and the profile service.
    pub location: String,
    pub webrtc_policy: WebrtcPolicy,
}

/// Map a fingerprint config to engine launch options. Pure: only empty/zero fields are skipped.
/// Flag spellings are the engine contract, not fingerprint logic — adjust here if the engine
/// changes, without touching any other layer.
///
/// `dialect` decides the vocabulary. [`FlagDialect::StockChromium`] emits only switches upstream
/// Chromium actually implements; every other fingerprint field emits **nothing**, because a
/// switch Chromium ignores would make Veil report a profile as spoofed when it is not.
/// [`FlagDialect::VeilPatched`] adds exactly eleven more: the Tier 1 five — `--fingerprint=<seed>`,
/// `--fingerprint-platform=` (from `os`, not from `fp`), `--timezone=`,
/// `--fingerprint-hardware-concurrency=` and `--disable-spoofing=` — plus Stage 6's six, each of
/// which waited for its patch to land *and* be measured: `--fingerprint-screen-width=`,
/// `--fingerprint-screen-height=` and `--fingerprint-device-scale-factor=` (T05, 6.11),
/// `--fingerprint-max-touch-points=` and `--fingerprint-device-memory=` (T13/T15, 6.18), and
/// `--fingerprint-location=` (T22, 6.22). That list is measured out of the patch set,
/// not inferred from the field names —
/// six of the switches this arm used to emit exist in no engine at all, and the three panel
/// switches were *declared* by `000` for five stages before `960` read one. One further switch
/// reaches the engine on this dialect but never through here: `--webrtc-ip=` is the launch paths'
/// (6.19's proxy mirror), because its value is the assigned proxy's exit address rather than a
/// `Fingerprint` field.
pub fn to_engine_args(fp: &Fingerprint, os: ProfileOs, dialect: FlagDialect) -> LaunchOptions {
    let patched = matches!(dialect, FlagDialect::VeilPatched);
    let mut args = Vec::new();
    let mut env = Vec::new();

    // Seed-based identity: the patch set derives every spoofed surface from one integer, so the
    // seed is only meaningful on an engine that carries it.
    if patched && fp.seed != 0 {
        args.push(format!("--fingerprint={}", fp.seed));
    }

    // The profile's OS, which `002`, `006` and `011` read. It is deliberately *not* a `Fingerprint`
    // field: `Profile::os` already holds the platform, and a second copy inside the fingerprint
    // would be one more pair of values that can contradict each other — the shape of bug this whole
    // stage has been removing. Emitted even for `windows`, because the switch also selects the
    // seed-derived platform *version* table (`002`), so leaving it off would make a Windows profile
    // report the host's real version while a macOS one reports a spoofed version.
    //
    // Measured: with `--fingerprint-platform=macos` the UA becomes `Macintosh; Intel Mac OS X
    // 10_15_7`, UA-CH reports `macOS` / `15.4.0` / `arm`, and `navigator.platform` becomes
    // `MacIntel` — coherent across all four. A literal `--user-agent=` reaches only the string and
    // *blanks* UA-CH's `platformVersion` and `architecture`, which is why `validate` now rejects it
    // alongside a seed.
    if patched && fp.seed != 0 {
        if let Some(platform) = os.as_fingerprint_platform() {
            args.push(format!("--fingerprint-platform={platform}"));
        }
    }

    // --- Switches upstream Chromium implements; emitted under every dialect. ---
    if !fp.user_agent.is_empty() {
        args.push(format!("--user-agent={}", fp.user_agent));
    }
    // Sets the window, NOT `screen.*` — a real `screen` spoof needs an engine patch.
    if fp.screen_width > 0 && fp.screen_height > 0 {
        args.push(format!(
            "--window-size={},{}",
            fp.screen_width, fp.screen_height
        ));
    }
    if !fp.locale.is_empty() {
        args.push(format!("--lang={}", fp.locale));
    }
    if !fp.languages.is_empty() {
        args.push(format!("--accept-lang={}", fp.languages.join(",")));
    }
    if !fp.timezone.is_empty() {
        if patched {
            // The patch set owns the timezone, so it must not also arrive through the
            // environment — one value, one channel.
            args.push(format!("--timezone={}", fp.timezone));
        } else {
            // Stock Chromium has no timezone switch. `TZ` is the only channel available, and it
            // is ignored on Windows (measured),
            // which is why the UI disables the field under this dialect.
            env.push(("TZ".to_string(), fp.timezone.clone()));
        }
    }
    if let Some(policy) = fp.webrtc_policy.as_ip_handling_policy() {
        args.push(format!("--webrtc-ip-handling-policy={policy}"));
    }

    // --- Switches that exist only in the Veil patch set. ---
    //
    // Exactly the vocabulary the patch set implements, read out of the patches rather than assumed.
    // Everything else the patch set spoofs
    // — canvas and audio noise, the WebGL unmasked vendor/renderer pair, the font list, `readPixels`
    // — is derived from the seed and has **no switch at all**, so there is nothing for this arm to
    // emit for those fields. (`000` also declares `--fingerprint-location`; it stayed unread until
    // T22's `970` was measured answering it, which is what admitted the arm below.)
    if patched {
        // `005` checks this switch **before** it looks at `--fingerprint`, and returns from it
        // directly, so it is honoured with or without a seed and overrides the seed-derived core
        // count when both are present. Note the `--fingerprint-` prefix: `--hardware-concurrency`
        // is not a switch any engine has.
        if fp.hardware_concurrency > 0 {
            args.push(format!(
                "--fingerprint-hardware-concurrency={}",
                fp.hardware_concurrency
            ));
        }
        // The declared panel, read by `960`. Gated on the seed as well as on the values, and that is
        // not symmetry with `--fingerprint-platform` for its own sake: `960` derives the work area,
        // the colour depth and the orientation from `--fingerprint-platform`, which this function
        // emits only alongside a seed. A panel with no platform claim would be given the Windows
        // taskbar inset on a macOS profile — a work area that agrees with neither side. Seedless, the
        // profile has no persona to be coherent with, so the honest emission is none.
        //
        // Gated on [`ProfileOs::has_fingerprint_platform`] for the same reason, one rung earlier: a
        // mobile profile gets no platform switch at all, so `960` would derive the display block's
        // companions from the *host* while the panel came from the stored value. `validate` refuses
        // the pair, and this arm repeats the condition so a hand-written command line cannot reach a
        // state the profile form is forbidden to store.
        if fp.seed != 0
            && os.has_fingerprint_platform()
            && fp.panel_width > 0
            && fp.panel_height > 0
        {
            args.push(format!("--fingerprint-screen-width={}", fp.panel_width));
            args.push(format!("--fingerprint-screen-height={}", fp.panel_height));
        }
        // Independent of the panel: a scale factor is a property of the display Veil is claiming and
        // `960` reads it on its own, so a profile may move the ratio without moving the panel. Two
        // decimals fixed, so one configuration always produces one byte-identical command line —
        // `1.5` and `1.50` would otherwise be the same profile with two argv. Same platform gate as
        // the panel: `(resolution:)` and `ScreenDetailed.devicePixelRatio` are read *beside* the
        // screen block, and a scale factor with no panel claim describes no display.
        if fp.seed != 0 && os.has_fingerprint_platform() && fp.device_pixel_ratio > 0.0 {
            args.push(format!(
                "--fingerprint-device-scale-factor={:.2}",
                fp.device_pixel_ratio
            ));
        }
        // `961` reads this in the browser process, and `input_persona.h` honours it with or without a
        // seed and on any claimed platform — `--fingerprint-hardware-concurrency`'s shape, for the
        // same reason: a count the user typed is not a cross-platform substitution, so it is not
        // gated on one. `None` is the opt-out and the default, and it leaves the host's input
        // configuration alone for a same-OS claim while `961`'s platform rung still answers 0 for a
        // macOS or Linux one. `Some(0)` is a *declaration* — "this machine has no touchscreen" — and
        // is why the field is an `Option` rather than a `u32` with 0 meaning unset.
        if let Some(points) = fp.max_touch_points {
            args.push(format!("--fingerprint-max-touch-points={points}"));
        }
        // `905` reads this in a renderer, also ungated on the seed, and falls back to the host's real
        // approximation when it is absent — which is what `005` has always done for the core count.
        // `validate` restricts the value to the five a desktop Chromium 151 can report, so this arm
        // does not clamp: a stored 6 never reaches here, and if a hand-written command line carries
        // one the engine falls through to the host rather than inventing a machine.
        if fp.device_memory > 0 {
            args.push(format!("--fingerprint-device-memory={}", fp.device_memory));
        }
        // `970` reads this in every renderer (`000` has propagated it since the port) and answers
        // `navigator.geolocation` with it — the last of `000`'s four declared-but-unread switches,
        // read and measured before this arm gained it (6.9's order). Ungated on the seed like
        // `--timezone=`, for the same reason: a position is part of the persona's *environment*,
        // not of its derived identity, and `validate` owns the agreement rules (coordinates need a
        // timezone; the timezone must agree with a tested proxy exit).
        if !fp.location.is_empty() {
            args.push(format!("--fingerprint-location={}", fp.location));
        }
        let opt_out = spoofing_opt_out(fp);
        if !opt_out.is_empty() {
            args.push(format!("--disable-spoofing={}", opt_out.join(",")));
        }
    }

    LaunchOptions { args, env }
}

/// The `--disable-spoofing` keys a stored fingerprint asks the engine to skip.
///
/// Tier 1 defined five keys — `audio`, `canvas`, `clientrects`, `font`, `gpu` — and every patch
/// that reads one does it with `GetSwitchValueASCII(kDisableSpoofing).find("<key>") == npos`, i.e.
/// **substring matching**, so a single comma-joined value covers any subset. **`clientrects` is no
/// longer one of them**: Stage 6 T09 dropped `014` for perturbing the half of the surface
/// fingerprinters ignore while adding a one-expression tell to the half they read, so nothing reads
/// that key and the vocabulary is four. The *readers* grew in
/// the same stage while the vocabulary shrank: `963` answers to `gpu`, `967` to `audio`, `969`'s codec
/// half to `gpu` and `972` to `font`, taking the patches behind the four keys from 8 to 11 — a key is
/// a surface group, not a patch, and phase 3 deliberately added none (6.14–6.16).
///
/// Veil has a field behind two of the four. `NoiseMode::Low` and `NoiseMode::High` are
/// indistinguishable to the engine, so the only part of the level Veil can honour is `Off` — and
/// this is where it is honoured. Leaving the surface spoofed while the UI says "Off" would be the
/// same class of lie as emitting a switch the engine ignores.
///
/// `font` (`006` + `906` + `972`) and `gpu` (`011` + `963` + `969`) have no Veil field yet, so Veil
/// never emits them; adding such a field means extending this function in the same change.
///
/// **`canvas` is wider than the canvas 2D surfaces.** `016-webgl-readPixels` reads the `canvas`
/// key as well, so opting out of canvas noise also stops the WebGL `readPixels` perturbation.
/// That grouping is the engine's, not Veil's, and the UI has to say so.
///
/// Emitted only alongside a seed: with no `--fingerprint=` nothing is spoofed in the first place,
/// so there is nothing to opt out of and the switch would be dead weight on the command line.
fn spoofing_opt_out(fp: &Fingerprint) -> Vec<&'static str> {
    if fp.seed == 0 {
        return Vec::new();
    }
    // Fixed order, so one configuration always produces one byte-identical switch.
    let mut keys = Vec::new();
    if fp.audio_noise == NoiseMode::Off {
        keys.push("audio");
    }
    if fp.canvas_noise == NoiseMode::Off {
        keys.push("canvas");
    }
    keys
}

/// The strip of a declared panel the claimed OS's shell reserves, in DIPs: `(top, bottom)`.
///
/// **This table is the Veil half of a pair.** The other half is
/// `components/ungoogled/display_persona.h` in `Veil-Chromium`, where `960` derives
/// `screen.avail*` from the same three numbers. Two copies of one table is normally the bug this
/// project keeps removing, and it is accepted here for a reason that does not generalise: the engine
/// has to derive the work area (it is what the page reads) and Veil has to *predict* it (it is what
/// decides whether a window fits at save time). Neither side can ask the other — the profile is
/// validated long before an engine is launched. Keep them in step so a drift is visible in
/// one place.
///
/// Each number is a statement about where an operating system puts its shell, never a hardware
/// spec, and each is far inside the 0–200 px `screen-os-chrome-is-plausible` allows. Windows 11
/// reserves 48 px at the bottom (measured on this build host: a 2560×1440 panel reports
/// `availHeight` 1392); macOS reserves 25 px at the top for the menu bar and is modelled with the
/// Dock auto-hidden, because the menu bar follows from the platform while a Dock height follows from
/// settings Veil has no field for; GNOME reserves 27 px at the top.
///
/// `Ios` gets the Windows strip because it emits no `--fingerprint-platform` at all (see
/// [`ProfileOs::as_fingerprint_platform`]), which is exactly what `960` falls back to. That arm
/// is unreachable from a stored profile — `validate` refuses a display block on a platform-less
/// OS and `to_engine_args` emits none of it (T24) — but the
/// function stays total rather than panicking on the value the enum keeps for stored-data
/// round-tripping, and the Windows strip stays the true answer for what the engine would do if a
/// hand-written command line carried one.
///
/// `Android` gets **no** inset since T24-a landed: Chrome
/// on a phone reports the work area equal to the full screen — the browser's own chrome is
/// window-level, not screen-level — and `990`'s display arm answers exactly that for the android
/// claim. The two halves of the pair stay in step.
pub const fn work_area_inset(os: ProfileOs) -> (u32, u32) {
    match os {
        ProfileOs::Macos => (25, 0),
        ProfileOs::Linux => (27, 0),
        ProfileOs::Windows | ProfileOs::Ios => (0, 48),
        ProfileOs::Android => (0, 0),
    }
}

/// The work area a declared panel leaves for a window, in DIPs: the panel minus
/// [`work_area_inset`]. `None` when the panel is unset or too small to hold a shell.
pub fn derived_work_area(fp: &Fingerprint, os: ProfileOs) -> Option<(u32, u32)> {
    if fp.panel_width == 0 || fp.panel_height == 0 {
        return None;
    }
    let (top, bottom) = work_area_inset(os);
    fp.panel_height
        .checked_sub(top + bottom)
        .filter(|height| *height > 0)
        .map(|height| (fp.panel_width, height))
}

/// The RAM sizes a Veil profile may declare, smallest first — a **subset** of what Chromium can
/// report, and the two numbers are different for a measured reason.
///
/// `ApproximatedDeviceMemory::CalculateAndSetApproximatedDeviceMemory` rounds the host's physical
/// memory to the *nearer* neighbouring power of two, divides by 1024 to get GB, and then clamps into
/// `[kMinMemory, kMaxMemory]` — `2.0` and `32.0` on desktop, `1.0` and `8.0` on Android, updated for
/// crbug.com/454354290. So a desktop Chrome 151 can report **five** values: 2, 4, 8, 16 and 32.
/// Measured 2026-08-28: stock Chrome 151 on the build host's 32 GiB reports `32`, which is also why the
/// engine-side counterpart's `min(value, 8)` clamp is against a stale spec.
///
/// **Veil offers three of the five, and the missing two are T14's answer rather than an oversight.**
/// V8 sizes its heap from the machine's physical memory — old generation
/// `clamp(physical / 2, 256 MB, 4 GB)` — so `performance.memory.jsHeapSizeLimit` saturates at 4 GiB
/// plus a young generation for any machine with 8 GB or more. A claim of 8, 16 or 32 therefore agrees
/// with the heap limit the host already reports; a claim of **2 or 4** does not, and closing that gap
/// would mean substituting a limit the runtime does not honour — a page can allocate past a spoofed-low
/// limit, or OOM before a spoofed-high one, which is a contradiction it can *test* rather than merely
/// read.
///
/// `coherence.mjs` keeps its own list of all five, because that rule scores what a page sees on any
/// engine and an unspoofed 2 GB machine is a real machine. This list is what a *profile* may declare.
///
/// The mirror is `@veil/shared`'s `DEVICE_MEMORY_GB`; the UI offers exactly this list.
pub const DEVICE_MEMORY_GB: [u8; 3] = [8, 16, 32];

/// Validate cross-field consistency. A default (all-empty) fingerprint is always valid; once a
/// field is set, related fields must agree so the engine isn't handed a contradictory identity.
///
/// Takes `os` because three of the rules below are about the machine the profile claims, and the
/// claim lives on `Profile::os` rather than in a `Fingerprint` field (5.5b: one identity, one
/// source). It is *not* a dialect: a stored profile must stay valid across an engine swap, so every
/// rule here is about the stored declaration and none about what the installed engine can emit.
pub fn validate(fp: &Fingerprint, os: ProfileOs) -> Result<(), ErrorCode> {
    // Languages must lead with the locale (a "en-US" locale advertising only "fr" is incoherent).
    if !fp.locale.is_empty() && !fp.languages.is_empty() && !fp.languages[0].starts_with(&fp.locale)
    {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // Plausible hardware: real devices report 1–64 logical cores here.
    if fp.hardware_concurrency > 64 {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // `navigator.deviceMemory` is not a free number, and it is narrower than Chromium's own range.
    // `ApproximatedDeviceMemory` can report 2, 4, 8, 16 or 32 on desktop; a Veil profile may declare
    // the top three, because V8 sizes its heap from physical memory and `jsHeapSizeLimit` saturates at
    // 8 GB — so a 2 or 4 GB claim contradicts the limit the host reports, and spoofing *that* is a
    // value the runtime would not honour (measured, 6.18). `DEVICE_MEMORY_GB` carries the reason, and
    // `js-heap-limit-follows-the-claimed-memory` is the capture-side half. `0` is "leave the host's".
    //
    // The bound this replaces was `> 64`, from the stage where the field reached nothing at all.
    if fp.device_memory != 0 && !DEVICE_MEMORY_GB.contains(&fp.device_memory) {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // No Mac has a touchscreen, and on this engine that is structural rather than a market fact:
    // macOS builds `ui/base/pointer/pointer_device_default.cc`, whose `MaxTouchPoints()` returns a
    // constant 0. A macOS profile declaring touch points would ask `961` to present a machine
    // Chromium cannot build, so it is refused at save time — the same move T06 made for the display
    // rules, and it is what keeps `macos-reports-no-touch-points` a rule about captures rather than
    // about profiles.
    //
    // 20 is a bound, not a table: Windows digitizers report 5, 10 or 20 and `SM_MAXIMUMTOUCHES` can
    // return more, but "which digitizers exist" is a spec sheet this file does not keep.
    if let Some(points) = fp.max_touch_points {
        if os == ProfileOs::Macos && points != 0 {
            return Err(ErrorCode::FingerprintInvalid);
        }
        if points > 20 {
            return Err(ErrorCode::FingerprintInvalid);
        }
    }
    // Both screen dimensions are set together or not at all.
    if (fp.screen_width == 0) != (fp.screen_height == 0) {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // Same for the declared panel, and for the same reason it is enforced in the engine: a spoofed
    // width beside the host's height is a machine narrower or taller than any that exists.
    if (fp.panel_width == 0) != (fp.panel_height == 0) {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // A panel small enough to leave no room for the OS shell is not a smaller laptop, and 16384 is
    // past every panel that ships. The floor is a real phone's panel, not a laptop's — T24-b's
    // phone rows are 360–412 CSS px wide — and bounds only: "which resolutions exist" is a spec
    // sheet this file does not keep.
    let panel_declared = fp.panel_width > 0;
    if panel_declared
        && (fp.panel_width < 320
            || fp.panel_height < 480
            || fp.panel_width > 16384
            || fp.panel_height > 16384)
    {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // A scale factor below 1 is a downscaled display no desktop reports, and above 4 is past every
    // shipping panel. `0.0` is "leave the host's" and is not a claim.
    if fp.device_pixel_ratio != 0.0 && !(1.0..=4.0).contains(&fp.device_pixel_ratio) {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // The display block reaches the engine only alongside a seed, because `960` derives the work
    // area, the colour depth and the orientation from `--fingerprint-platform` and `to_engine_args`
    // emits *that* only with a seed. Same shape as the noise rule below: a stored request the engine
    // provably cannot honour is an incoherent profile, not a harmless one.
    if (panel_declared || fp.device_pixel_ratio != 0.0) && fp.seed == 0 {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // The same rule for the other way the platform switch can be absent. It is emitted only when
    // `Profile::os` has a platform to claim, and `ios` has none (6.24: no precedent anywhere, no
    // path to one — `android` left this arm when the port landed, 6.26), so an iOS profile
    // reaches `960` with no platform at all: the work area, the colour depth and the orientation
    // would be derived from the *host* while the panel came from the store. The engine ignores
    // the display block for such a profile — `to_engine_args` emits none of it — and a stored
    // request the engine provably cannot honour is an incoherent profile, exactly as seedless is.
    //
    // The stored half of the UI gate, and it has a stated cost: a profile written before the
    // change still opens, but it cannot be *saved* while it carries a display block, and the field
    // is disabled so the way out is the OS control. That is the honest outcome for a configuration
    // the engine ignores, and it is the same shape the seed rule above has always had; the
    // alternative, letting the value through, would be a stored request the launch path silently
    // drops.
    if (panel_declared || fp.device_pixel_ratio != 0.0) && !os.has_fingerprint_platform() {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // T24-b (6.26): an android claim IS a phone, and the phone rows are atomic. The platform
    // switch is emitted for android now, and `960`'s android arm derives the work area from the
    // panel — so a panel-less platform describes no machine, and the panel must be PORTRAIT
    // (every phone panel is taller than wide; a landscape panel beside an android claim is a
    // desktop monitor wearing a phone's identity). Touch points are REQUIRED and non-zero — a
    // phone has a touchscreen, `961`'s android rung answers the count, and
    // `android-carries-a-touchscreen` scores the capture — the macOS rule above, pointed the
    // other way. The scale factor is required with the panel, because `(resolution:)` and
    // `ScreenDetailed.devicePixelRatio` are read beside the screen block; the same reasoning
    // that makes a panel without a scale factor fail the macOS rule above.
    //
    // The panel values themselves come from the same device table the engine reads
    // (`crates/core/src/android_device.rs` mirrors the engine's `kAndroidDevices`), which is
    // what `randomize` offers and what keeps the window, the panel and the DPR one machine.
    if os == ProfileOs::Android && fp.seed != 0 {
        if !panel_declared || fp.device_pixel_ratio == 0.0 {
            return Err(ErrorCode::FingerprintInvalid);
        }
        if fp.panel_height <= fp.panel_width {
            return Err(ErrorCode::FingerprintInvalid);
        }
        if !matches!(fp.max_touch_points, Some(points) if points > 0) {
            return Err(ErrorCode::FingerprintInvalid);
        }
    }
    // Apple Silicon implies Retina, and this is where the profile is rejected rather than the
    // snapshot. `002` reports UA-CH `architecture: arm` for **every** `--fingerprint-platform=macos`
    // profile — unconditionally, one branch, no seed involvement — so a macOS profile is always an
    // Apple Silicon claim, and no Apple Silicon Mac has shipped a display below 2×. A declared panel
    // with no scale factor fails this too, on purpose: it would report the host's 1× beside a
    // 1470×956 panel, which is the exact pair that made the `-1.1001` snapshot describe a machine
    // that does not exist (`coherence.mjs` → `apple-silicon-implies-retina`).
    if os == ProfileOs::Macos
        && (panel_declared || fp.device_pixel_ratio != 0.0)
        && fp.device_pixel_ratio < 2.0
    {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // The window has to fit the work area the panel leaves. Assertion 6 R2
    // (`window-fits-the-work-area`) scored as a floor while nothing drove `screen.*`; now that `960`
    // does, a window larger than its own declared work area is a defect, and the cheapest place to
    // make it impossible is here. This is also why `960` does **not** clamp `outerWidth` /
    // `outerHeight` the way its counterpart does: the window stays a true measurement because the
    // configuration that would need clamping cannot be saved.
    if let Some((work_width, work_height)) = derived_work_area(fp, os) {
        if fp.screen_width > work_width || fp.screen_height > work_height {
            return Err(ErrorCode::FingerprintInvalid);
        }
    } else if panel_declared {
        // The panel passed the bounds above but its shell strip consumed all of it.
        return Err(ErrorCode::FingerprintInvalid);
    }
    // Timezone must be a plausible IANA zone id. This became a real coherence rule with Tier 1
    // patch `018-timezone`: before it, the value reached nothing — Windows Chromium ignores `TZ`
    // (Stage 1.6) — and a malformed string was inert. Now `--timezone=` is handed to
    // `TimeZoneController`, which pushes it straight into ICU and notifies V8, so a value ICU
    // cannot parse produces a profile whose clock disagrees with itself. Shape only: shipping an
    // IANA database to check membership would put a second, staler copy of tzdata in the repo.
    if !fp.timezone.is_empty() && !is_iana_timezone(&fp.timezone) {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // A declared position is the user's to give — Veil is offline and holds no geo-IP table, so
    // nothing derives coordinates from the proxy exit — and the engine applies the same WGS84
    // bounds `ValidateGeoposition()` applies to a real answer (970). A value the engine would
    // silently ignore is the flag-dialect lie arriving through the stored profile, so the shape is
    // refused here rather than at launch.
    if !fp.location.is_empty() {
        let Some((latitude, longitude)) = parse_location(&fp.location) else {
            return Err(ErrorCode::FingerprintInvalid);
        };
        if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
            return Err(ErrorCode::FingerprintInvalid);
        }
        // Coordinates locate the persona in space; the timezone locates it in time. One without
        // the other is half a machine, and `misc.geolocation` sitting beside an `Intl` clock three
        // timezones away is exactly the contradiction assertion 6 exists to score.
        if fp.timezone.is_empty() {
            return Err(ErrorCode::FingerprintInvalid);
        }
    }
    // Canvas / audio noise is seed-derived in the engine: the patch set reads `--fingerprint=<seed>`
    // and nothing else. `003-audio-fingerprint` returns a zero perturbation with no seed, and
    // `012-canvas-get-image-data` never calls the shuffle at all, so a profile asking for noise
    // while leaving `seed` at 0 gets none — the exact shape of lie the flag dialect exists to
    // prevent, arriving from the other side. There is no `--canvas-noise` / `--audio-noise` in
    // Tier 1, so the level (`Low` / `High`) is not distinguishable either; only "on" and "off" are,
    // and "on" means "seeded". `Off` is not merely absent from the command line: with a seed it is
    // emitted as a `--disable-spoofing=` key, so the surface the UI reports as off really is off.
    if (fp.canvas_noise != NoiseMode::Off || fp.audio_noise != NoiseMode::Off) && fp.seed == 0 {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // One identity source at a time. A non-zero seed means the engine derives the identity -- UA,
    // UA-CH, platform, platform version and architecture all come from `--fingerprint=` plus
    // `--fingerprint-platform=`. A literal `user_agent` reaches only the UA *string*, and measured
    // against the engine it is worse than merely redundant: Chromium stops populating the UA-CH
    // high-entropy hints, so `platformVersion` and `architecture` come back as empty strings, which
    // no real browser returns. A macOS UA string beside UA-CH `platform: Windows` and two blank
    // hints is a louder tell than either value alone (5.5b).
    //
    // Dialect-independent on purpose, even though a stock engine ignores the seed: `validate` takes
    // no dialect so a stored profile stays valid across an engine swap, and the honest reading of
    // "seed + literal UA" is that the user asked for two identities either way. Clear one of them.
    if !fp.user_agent.is_empty() && fp.seed != 0 {
        return Err(ErrorCode::FingerprintInvalid);
    }
    // No further rule is added by the switch-vocabulary correction, and the reason is structural
    // rather than an oversight: the two contradictions it exposes are both dialect-specific, and
    // `validate` deliberately takes no dialect — a stored profile must stay valid across an engine
    // swap. `device_memory` is contradicted only by `veil-patched` (`005` returns 8 regardless),
    // and the `Low` / `High` distinction is meaningless only there. Neither is expressible here,
    // and neither is a *stored* incoherence: 32 GB is a value real Chromium reports (the zero-patch
    // floor on this host reports exactly that), so rejecting it would be wrong on its own terms.
    // Both belong to the UI's "spoofed, but not configurable" category instead.
    Ok(())
}

/// `Area/Location`, `Area/Sub/Location`, or one of the two bare ids that carry no slash.
///
/// Deliberately structural. The point is to reject `""`-adjacent junk, a POSIX form such as
/// `EST5EDT`, a Windows display name such as `China Standard Time`, and anything with whitespace or
/// a path separator — not to be an authority on which zones exist.
fn is_iana_timezone(value: &str) -> bool {
    if value == "UTC" || value == "GMT" {
        return true;
    }
    let mut parts = value.split('/');
    let area = parts.next().unwrap_or_default();
    let rest: Vec<&str> = parts.collect();
    if rest.is_empty() || rest.len() > 2 {
        return false;
    }
    let component_ok = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '+')
    };
    // The area is always alphabetic (`America`, `Asia`, `Etc`); only the tail may carry digits,
    // as in `Etc/GMT+8`.
    area.chars().all(|c| c.is_ascii_alphabetic() || c == '_')
        && !area.is_empty()
        && rest.iter().all(|part| component_ok(part))
}

/// The two halves of a stored `"lat,lng"` position, the exact string `970` reads.
///
/// Structural, like [`is_iana_timezone`]: a decimal-signature check that refuses junk and
/// comma-free input, not an authority on which coordinates are on land. `±90,±180` bounds are
/// enforced by the caller, mirroring the engine's own guard in `970`.
fn parse_location(value: &str) -> Option<(f64, f64)> {
    let (latitude, longitude) = value.split_once(',')?;
    let latitude = latitude.trim().parse::<f64>().ok()?;
    let longitude = longitude.trim().parse::<f64>().ok()?;
    Some((latitude, longitude))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every test that does not care about the platform switch goes through here. `Windows` is what
    /// `ProfileOs::default()` gives a profile, so this is the ordinary case; the tests that do care
    /// call `to_engine_args` directly with the OS they mean.
    fn opts(fp: &Fingerprint, dialect: FlagDialect) -> LaunchOptions {
        to_engine_args(fp, ProfileOs::Windows, dialect)
    }

    /// Same arrangement for `validate`, which took no OS before T06. The rules that read it are the
    /// display ones, so every pre-existing case is a Windows case; the macOS rules are tested by
    /// calling `validate` directly.
    fn valid(fp: &Fingerprint) -> Result<(), ErrorCode> {
        validate(fp, ProfileOs::Windows)
    }

    /// Every switch that does not exist in upstream Chromium. `stock-chromium` must never emit
    /// one of these: Chromium ignores unknown switches silently, so each is a potential lie.
    const NOT_IN_STOCK_CHROMIUM: [&str; 19] = [
        // Tier 1's real vocabulary — implemented by the patch set, absent from stock.
        "--fingerprint",
        "--timezone",
        "--fingerprint-hardware-concurrency",
        "--disable-spoofing",
        // Spellings that exist in no engine at all; see `NOT_IN_TIER_ONE`.
        "--canvas-noise",
        "--audio-noise",
        "--webgl-vendor",
        "--webgl-renderer",
        "--fonts",
        "--hardware-concurrency",
        "--device-memory",
        "--webrtc-policy",
        // Shipped for three stages and read by nothing the `chrome` binary runs. It *is* a Chromium
        // switch (`content/public/common/content_switches.cc`), which is why the mistake survived —
        // but only `content_shell` and `headless/lib` read it, while `chrome/` maps
        // `--webrtc-ip-handling-policy` onto the pref. The list meant to catch exactly this class of
        // bug did not contain the one live instance of it.
        "--force-webrtc-ip-handling-policy",
        "--fingerprint-screen-width",
        "--fingerprint-screen-height",
        "--fingerprint-device-scale-factor",
        // Veil-authored and added in Stage 6 phase 4 — `961` and `905`. Neither is one of `000`'s
        // twelve, so upstream has never had them under any spelling.
        "--fingerprint-max-touch-points",
        "--fingerprint-device-memory",
        // `000`'s own twelfth declaration, read by `970` since Stage 6's T22. A `veil-patched`
        // switch now, but still one stock Chromium has never had.
        "--fingerprint-location",
    ];

    /// Switches **no patch in the set reads**, so `veil-patched` must not emit them either. Six were
    /// emitted by the `veil-patched` arm before the 5.4 correction and one was the misspelling of
    /// a switch that does exist; `--fingerprint-location` was declared by `000` and propagated to
    /// the renderer but read by nothing until Stage 6's T22.
    ///
    /// Four entries **left** this list as their patches landed and were measured — the three panel
    /// switches in T06 and `--fingerprint-location` in T22 — and they left it in the direction
    /// §1 requires: the patch exists, builds, and its surface was measured to move before the arm
    /// above gained it. They stay in `NOT_IN_STOCK_CHROMIUM`, because stock Chromium still has no
    /// such switch.
    ///
    /// This is the mirror image of `stock_never_emits_a_switch_chromium_does_not_have`, and it is
    /// the test that would have caught the flip going out with a lie in it.
    const NOT_IN_TIER_ONE: [&str; 7] = [
        "--canvas-noise",
        "--audio-noise",
        "--webgl-vendor",
        "--webgl-renderer",
        "--fonts",
        "--device-memory",
        "--hardware-concurrency",
    ];

    /// Every field set, including the `user_agent` + `seed` pair `validate` now rejects (5.5b). That
    /// is deliberate: `to_engine_args` is a pure mapper and must produce a correct command line for
    /// *any* input, coherent or not, because it also runs on stored profiles written before a rule
    /// existed. Coherence is `validate`'s job and is tested separately.
    fn fully_configured() -> Fingerprint {
        Fingerprint {
            seed: 42,
            user_agent: "UA/1".into(),
            screen_width: 1920,
            screen_height: 1080,
            // A panel the 1920×1080 window above actually fits inside, so this fixture stays usable
            // for the coherence tests as well as the mapping ones.
            panel_width: 2560,
            panel_height: 1440,
            device_pixel_ratio: 1.0,
            timezone: "America/New_York".into(),
            locale: "en-US".into(),
            languages: vec!["en-US".into(), "en".into()],
            webgl_vendor: "Google Inc.".into(),
            webgl_renderer: "ANGLE".into(),
            canvas_noise: NoiseMode::Low,
            audio_noise: NoiseMode::High,
            fonts: vec!["Arial".into(), "Roboto".into()],
            hardware_concurrency: 8,
            device_memory: 16,
            max_touch_points: Some(10),
            // A position that agrees with the fixture's timezone, so the fixture stays usable for
            // the coherence tests: New York is at 40.71°N, 74.01°W.
            location: "40.7128,-74.0060".into(),
            webrtc_policy: WebrtcPolicy::ProxyOnly,
        }
    }

    #[test]
    fn default_emits_nothing_under_either_dialect() {
        for dialect in [FlagDialect::StockChromium, FlagDialect::VeilPatched] {
            let options = opts(&Fingerprint::default(), dialect);
            assert!(options.args.is_empty(), "{dialect:?} emitted args");
            assert!(options.env.is_empty(), "{dialect:?} emitted env");
        }
    }

    #[test]
    fn stock_emits_only_switches_upstream_chromium_implements() {
        let options = opts(&fully_configured(), FlagDialect::StockChromium);
        assert_eq!(
            options.args,
            vec![
                "--user-agent=UA/1",
                "--window-size=1920,1080",
                "--lang=en-US",
                "--accept-lang=en-US,en",
                "--webrtc-ip-handling-policy=disable_non_proxied_udp",
            ]
        );
    }

    #[test]
    fn stock_never_emits_a_switch_chromium_does_not_have() {
        // Assert over the whole arg list, not a sample: a single phantom switch is enough to
        // make the UI's "spoofed" claim false.
        let options = opts(&fully_configured(), FlagDialect::StockChromium);
        for arg in &options.args {
            let name = arg.split('=').next().unwrap_or(arg);
            assert!(
                !NOT_IN_STOCK_CHROMIUM.contains(&name),
                "stock-chromium emitted {arg}"
            );
        }
    }

    #[test]
    fn stock_carries_timezone_in_env_not_argv() {
        let options = opts(&fully_configured(), FlagDialect::StockChromium);
        assert_eq!(
            options.env,
            vec![("TZ".to_string(), "America/New_York".to_string())]
        );
        for arg in &options.args {
            assert!(!arg.starts_with("--timezone"), "unexpected {arg}");
            assert!(!arg.starts_with("--tz"), "unexpected {arg}");
        }
    }

    /// The whole point of 5.4: this list *is* the patch set's vocabulary. `fully_configured()` sets
    /// every field, including three the patch set has no switch for, and they must produce nothing.
    #[test]
    fn veil_patched_emits_exactly_the_tier_one_vocabulary() {
        let options = opts(&fully_configured(), FlagDialect::VeilPatched);
        assert_eq!(
            options.args,
            vec![
                "--fingerprint=42",
                "--fingerprint-platform=windows",
                "--user-agent=UA/1",
                "--window-size=1920,1080",
                "--lang=en-US",
                "--accept-lang=en-US,en",
                "--timezone=America/New_York",
                "--webrtc-ip-handling-policy=disable_non_proxied_udp",
                "--fingerprint-hardware-concurrency=8",
                "--fingerprint-screen-width=2560",
                "--fingerprint-screen-height=1440",
                "--fingerprint-device-scale-factor=1.00",
                "--fingerprint-max-touch-points=10",
                "--fingerprint-device-memory=16",
                "--fingerprint-location=40.7128,-74.0060",
            ]
        );
    }

    #[test]
    fn veil_patched_never_emits_a_switch_no_tier_one_patch_reads() {
        // Over both dialects and over the whole arg list. A switch with no patch behind it is the
        // Stage 1 bug in mirror image: the engine ignores it and the UI reports protection.
        for dialect in [FlagDialect::StockChromium, FlagDialect::VeilPatched] {
            let options = opts(&fully_configured(), dialect);
            for arg in &options.args {
                let name = arg.split('=').next().unwrap_or(arg);
                assert!(
                    !NOT_IN_TIER_ONE.contains(&name),
                    "{dialect:?} emitted {arg}, which no Tier 1 patch reads"
                );
            }
        }
    }

    /// `005` reads `--fingerprint-hardware-concurrency`. The old arm emitted
    /// `--hardware-concurrency`, which no engine has ever had — the one misspelling in the set,
    /// and silently dead rather than loudly wrong.
    #[test]
    fn hardware_concurrency_carries_the_fingerprint_prefix() {
        let fp = Fingerprint {
            hardware_concurrency: 12,
            ..Default::default()
        };
        assert_eq!(
            opts(&fp, FlagDialect::VeilPatched).args,
            vec!["--fingerprint-hardware-concurrency=12"]
        );
        // `005` checks the switch before it looks at `--fingerprint`, so no seed is required.
        assert!(opts(&fp, FlagDialect::StockChromium).args.is_empty());
    }

    /// `Some(0)` and `None` are different profiles, which is the whole reason the field is an
    /// `Option`: a Mac declares zero touch points and the sentinel cannot also mean zero.
    #[test]
    fn max_touch_points_distinguishes_unset_from_a_declared_zero() {
        let unset = Fingerprint::default();
        assert!(opts(&unset, FlagDialect::VeilPatched).args.is_empty());

        let declared_zero = Fingerprint {
            max_touch_points: Some(0),
            ..Default::default()
        };
        assert_eq!(
            opts(&declared_zero, FlagDialect::VeilPatched).args,
            vec!["--fingerprint-max-touch-points=0"]
        );

        // Ungated on the seed, like `--fingerprint-hardware-concurrency`: `961` honours a typed count
        // on any claim. And absent from stock, which has no such switch under any spelling.
        assert!(opts(&declared_zero, FlagDialect::StockChromium)
            .args
            .is_empty());
    }

    /// T15's field chain, end to end: the stored value reaches the engine, and `0` is the opt-out
    /// that leaves the host's own approximation in place.
    #[test]
    fn device_memory_reaches_the_engine_and_zero_means_the_host() {
        for gb in DEVICE_MEMORY_GB {
            let fp = Fingerprint {
                device_memory: gb,
                ..Default::default()
            };
            assert_eq!(
                opts(&fp, FlagDialect::VeilPatched).args,
                vec![format!("--fingerprint-device-memory={gb}")]
            );
            assert!(opts(&fp, FlagDialect::StockChromium).args.is_empty());
            assert!(valid(&fp).is_ok(), "{gb} GB should be storable");
        }
        let host = Fingerprint {
            device_memory: 0,
            ..Default::default()
        };
        assert!(opts(&host, FlagDialect::VeilPatched).args.is_empty());
    }

    /// A profile may declare three of the five sizes Chromium can report, and 2 and 4 are refused for
    /// a reason rather than a bound: V8's heap limit saturates at 8 GB, so a smaller claim contradicts
    /// the `jsHeapSizeLimit` the page also reads (6.18). `64` was storable until T15 — the old bound
    /// was `> 64`, from the stage where the field reached nothing at all and any number was equally
    /// inert.
    #[test]
    fn device_memory_outside_what_a_profile_may_declare_is_refused() {
        for gb in [1u8, 2, 3, 4, 6, 12, 24, 64, 255] {
            let fp = Fingerprint {
                device_memory: gb,
                ..Default::default()
            };
            assert_eq!(
                valid(&fp),
                Err(ErrorCode::FingerprintInvalid),
                "{gb} GB is not a size a Veil profile may declare"
            );
        }
    }

    /// T22's field chain, end to end. The stored `"lat,lng"` string reaches the engine byte-for-byte
    /// — it *is* the switch value — and an empty field emits nothing. Ungated on the seed like
    /// `--timezone=`, and absent from stock under every spelling.
    #[test]
    fn location_reaches_the_engine_and_empty_emits_nothing() {
        let fp = Fingerprint {
            location: "40.7128,-74.0060".into(),
            ..Default::default()
        };
        assert_eq!(
            opts(&fp, FlagDialect::VeilPatched).args,
            vec!["--fingerprint-location=40.7128,-74.0060"]
        );
        assert!(opts(&fp, FlagDialect::StockChromium).args.is_empty());
        assert!(opts(&Fingerprint::default(), FlagDialect::VeilPatched)
            .args
            .is_empty());
    }

    /// The coordinates are the user's to give — Veil holds no geo-IP table — but a value the
    /// engine would refuse or ignore is the flag-dialect lie arriving through the stored profile.
    /// And coordinates without a timezone are half a machine: the persona has a place but no clock
    /// the geolocation could agree with.
    #[test]
    fn a_declared_location_must_parse_and_carry_a_timezone() {
        let base = Fingerprint {
            timezone: "America/New_York".into(),
            ..Default::default()
        };
        for good in [
            "40.7128,-74.0060",
            " 0 , 0 ",
            "-33.8688,151.2093",
            "0,180",
            "-90,180",
        ] {
            let fp = Fingerprint {
                location: good.into(),
                ..base.clone()
            };
            assert!(valid(&fp).is_ok(), "should accept {good:?}");
        }
        for bad_location in [
            "40.7128",
            "40.7128,",
            ",-74.0060",
            "a,b",
            "40.7128,-74.0060,3",
        ] {
            let fp = Fingerprint {
                location: bad_location.into(),
                ..base.clone()
            };
            assert_eq!(
                valid(&fp),
                Err(ErrorCode::FingerprintInvalid),
                "should reject {bad_location:?}"
            );
        }
        // Outside WGS84 — the same bounds `970` applies before answering, because a position no
        // receiver accepts is worse than none.
        for out_of_range in ["90.5,0", "0,-180.5", "91,181"] {
            let fp = Fingerprint {
                location: out_of_range.into(),
                ..base.clone()
            };
            assert_eq!(valid(&fp), Err(ErrorCode::FingerprintInvalid));
        }
        // Coordinates without a timezone are refused; the timezone alone stays fine.
        let no_timezone = Fingerprint {
            location: "40.7128,-74.0060".into(),
            ..Default::default()
        };
        assert_eq!(valid(&no_timezone), Err(ErrorCode::FingerprintInvalid));
    }

    /// `macos-reports-no-touch-points` is a capture rule; this is the save-time half, so the
    /// contradiction cannot be stored in the first place. Same move T06 made for the display block.
    #[test]
    fn a_macos_profile_cannot_declare_touch_points() {
        let touching = Fingerprint {
            max_touch_points: Some(10),
            ..Default::default()
        };
        assert_eq!(
            validate(&touching, ProfileOs::Macos),
            Err(ErrorCode::FingerprintInvalid)
        );
        // Zero is the only count a Mac can report, and unset is always fine.
        for allowed in [Some(0), None] {
            let fp = Fingerprint {
                max_touch_points: allowed,
                ..Default::default()
            };
            assert!(validate(&fp, ProfileOs::Macos).is_ok(), "{allowed:?}");
        }
        // Windows and Linux touch laptops are ordinary, so the same 10 is storable there.
        for os in [ProfileOs::Windows, ProfileOs::Linux] {
            assert!(validate(&touching, os).is_ok(), "{os:?}");
        }
        // A bound, not a table.
        let too_many = Fingerprint {
            max_touch_points: Some(21),
            ..Default::default()
        };
        assert_eq!(valid(&too_many), Err(ErrorCode::FingerprintInvalid));
    }

    /// `NoiseMode::Off` is the only part of the level a Tier 1 engine can be told, and it is told
    /// through `--disable-spoofing`. Substring matching in the patches makes one comma-joined
    /// value work for any subset.
    #[test]
    fn noise_off_becomes_a_disable_spoofing_key() {
        let cases: [(NoiseMode, NoiseMode, Option<&str>); 4] = [
            // Both off: the seed still drives the font list, the GPU name and its capability block,
            // so the profile is not "unspoofed" — only these two surfaces are opted out of.
            (
                NoiseMode::Off,
                NoiseMode::Off,
                Some("--disable-spoofing=audio,canvas"),
            ),
            (
                NoiseMode::Off,
                NoiseMode::Low,
                Some("--disable-spoofing=audio"),
            ),
            (
                NoiseMode::High,
                NoiseMode::Off,
                Some("--disable-spoofing=canvas"),
            ),
            (NoiseMode::Low, NoiseMode::High, None),
        ];
        for (audio_noise, canvas_noise, expected) in cases {
            let fp = Fingerprint {
                seed: 7,
                audio_noise,
                canvas_noise,
                ..Default::default()
            };
            let args = opts(&fp, FlagDialect::VeilPatched).args;
            // `--fingerprint-platform=windows` rides along with every seed; see `to_engine_args`.
            match expected {
                Some(switch) => assert_eq!(
                    args,
                    vec!["--fingerprint=7", "--fingerprint-platform=windows", switch],
                    "{audio_noise:?}/{canvas_noise:?}"
                ),
                None => assert_eq!(
                    args,
                    vec!["--fingerprint=7", "--fingerprint-platform=windows"],
                    "{audio_noise:?}/{canvas_noise:?} should opt out of nothing"
                ),
            }
        }
    }

    /// Without a seed nothing is spoofed, so there is nothing to opt out of — and a default
    /// profile must stay silent under both dialects (asserted separately above, but this is the
    /// case that would regress if the opt-out were emitted unconditionally).
    #[test]
    fn seedless_noise_off_emits_no_opt_out() {
        let fp = Fingerprint {
            seed: 0,
            canvas_noise: NoiseMode::Off,
            audio_noise: NoiseMode::Off,
            hardware_concurrency: 8,
            ..Default::default()
        };
        assert_eq!(
            opts(&fp, FlagDialect::VeilPatched).args,
            vec!["--fingerprint-hardware-concurrency=8"]
        );
    }

    /// The patch set has no switch for these three, and the `veil-patched` arm used to emit all of
    /// them. Setting only the unsupported fields must produce an empty command line under both
    /// dialects.
    ///
    /// `device_memory` was the fourth entry here until T15 gave it `--fingerprint-device-memory`
    /// (`905`), which is exactly the direction §1's three-mirror rule allows a field to move: patch
    /// first, measured, then this test and `fingerprintFieldState()` in the same commit.
    #[test]
    fn fields_with_no_switch_behind_them_emit_nothing() {
        let fp = Fingerprint {
            webgl_vendor: "Google Inc.".into(),
            webgl_renderer: "ANGLE".into(),
            fonts: vec!["Arial".into()],
            ..Default::default()
        };
        for dialect in [FlagDialect::StockChromium, FlagDialect::VeilPatched] {
            assert!(
                opts(&fp, dialect).args.is_empty(),
                "{dialect:?} emitted something for a field with no switch"
            );
        }
    }

    #[test]
    fn veil_patched_keeps_timezone_out_of_env() {
        let options = opts(&fully_configured(), FlagDialect::VeilPatched);
        assert!(options.env.is_empty());
    }

    #[test]
    fn seed_zero_emits_no_fingerprint_flag() {
        let fp = Fingerprint {
            seed: 0,
            ..Default::default()
        };
        assert!(opts(&fp, FlagDialect::VeilPatched).args.is_empty());
        let fp_seeded = Fingerprint {
            seed: 99999,
            ..Default::default()
        };
        // A seed with the default noise mode is a deliberate "spoof, but not canvas and not
        // audio": `NoiseMode::Off` is a choice in the UI's select, not an "unset" sentinel, so it
        // is honoured rather than quietly upgraded. `runtime::randomize` sets `Low` whenever it
        // sets a seed, so a randomized profile does not land here.
        assert_eq!(
            opts(&fp_seeded, FlagDialect::VeilPatched).args,
            vec![
                "--fingerprint=99999",
                "--fingerprint-platform=windows",
                "--disable-spoofing=audio,canvas"
            ]
        );
        // The seed is a patch-set switch; a stock engine would ignore it silently. So is the
        // opt-out, which is why neither appears here.
        assert!(opts(&fp_seeded, FlagDialect::StockChromium).args.is_empty());
    }

    /// `--fingerprint-platform` is what makes a cross-platform persona coherent. Measured (5.5b):
    /// with `macos` the UA becomes `Macintosh; Intel Mac OS X 10_15_7`, UA-CH reports
    /// `macOS` / `15.4.0` / `arm` and `navigator.platform` becomes `MacIntel`.
    #[test]
    fn platform_switch_follows_the_profile_os() {
        let fp = Fingerprint {
            seed: 7,
            ..Default::default()
        };
        for (os, expected) in [
            (ProfileOs::Windows, Some("--fingerprint-platform=windows")),
            (ProfileOs::Macos, Some("--fingerprint-platform=macos")),
            (ProfileOs::Linux, Some("--fingerprint-platform=linux")),
            // `android` joined the implemented set in 6.26 (the `990` port); `ios` has no
            // precedent anywhere (6.24), so its switch is withheld rather than emitted with a
            // value the engine would ignore — a switch name that looks supported and is not is
            // harder to notice than one that does not exist.
            (ProfileOs::Android, Some("--fingerprint-platform=android")),
            (ProfileOs::Ios, None),
        ] {
            let args = to_engine_args(&fp, os, FlagDialect::VeilPatched).args;
            match expected {
                Some(switch) => assert!(args.contains(&switch.to_string()), "{os:?} -> {args:?}"),
                None => assert!(
                    !args.iter().any(|a| a.starts_with("--fingerprint-platform")),
                    "{os:?} must emit no platform switch, got {args:?}"
                ),
            }
        }
    }

    /// The platform is an identity, so it only means something once there is an identity to derive:
    /// no seed, no `--fingerprint-platform`. It is also never emitted on a stock engine, which has
    /// no such switch.
    #[test]
    fn platform_switch_needs_a_seed_and_the_patched_dialect() {
        let seedless = Fingerprint::default();
        for dialect in [FlagDialect::StockChromium, FlagDialect::VeilPatched] {
            assert!(
                to_engine_args(&seedless, ProfileOs::Macos, dialect)
                    .args
                    .is_empty(),
                "{dialect:?} emitted something for a seedless profile"
            );
        }
        let seeded = Fingerprint {
            seed: 7,
            ..Default::default()
        };
        assert!(
            to_engine_args(&seeded, ProfileOs::Macos, FlagDialect::StockChromium)
                .args
                .is_empty()
        );
    }

    /// T24-b (6.26): the two mobile claims split. An **android** profile emits its platform switch
    /// and the display block beside it — `990` implements every surface the claim touches, and
    /// `validate` requires the panel, the scale factor and a non-zero touch count to ride along.
    /// An **ios** profile still emits nothing on behalf of its platform claim: no patch anywhere
    /// handles the value (6.24), so the switch is withheld along with the display block `960`
    /// derives from it. The seed and the switches that are not platform-derived still reach the
    /// engine on either value, because a stored profile is still a profile.
    #[test]
    fn mobile_platforms_emit_their_claimed_half() {
        let android = Fingerprint {
            seed: 424242,
            panel_width: 412,
            panel_height: 892,
            device_pixel_ratio: 2.62,
            max_touch_points: Some(5),
            hardware_concurrency: 8,
            ..Default::default()
        };
        let args = to_engine_args(&android, ProfileOs::Android, FlagDialect::VeilPatched).args;
        for expected in [
            "--fingerprint-platform=android",
            "--fingerprint-screen-width=412",
            "--fingerprint-screen-height=892",
            "--fingerprint-device-scale-factor=2.62",
            "--fingerprint-max-touch-points=5",
            "--fingerprint=424242",
            "--fingerprint-hardware-concurrency=8",
        ] {
            assert!(
                args.contains(&expected.to_string()),
                "{expected:?} vs {args:?}"
            );
        }
        // The ios control: the same fingerprint withholds the platform half entirely — the value
        // no engine implements — while the non-platform switches still reach the engine.
        let ios_args = to_engine_args(&android, ProfileOs::Ios, FlagDialect::VeilPatched).args;
        for prefix in [
            "--fingerprint-platform",
            "--fingerprint-screen-",
            "--fingerprint-device-scale-factor",
        ] {
            assert!(
                !ios_args.iter().any(|arg| arg.starts_with(prefix)),
                "ios emitted {prefix}, got {ios_args:?}"
            );
        }
        assert!(
            ios_args.contains(&"--fingerprint=424242".to_string()),
            "{ios_args:?}"
        );
        // The desktop control: the same fingerprint on Windows keeps both switches, so this test is
        // about the platform claim and not about the values.
        let windows = to_engine_args(&android, ProfileOs::Windows, FlagDialect::VeilPatched).args;
        assert!(windows.contains(&"--fingerprint-platform=windows".to_string()));
        assert!(windows.contains(&"--fingerprint-screen-width=412".to_string()));
    }

    /// The stored half of the same split. An ios profile that asks for a display block is refused
    /// at save time — the value reaches nothing. An android profile is required to carry the whole
    /// block: the panel, the scale factor and a non-zero touch count are what make the claim a
    /// machine (6.26), and each missing piece is refused the same way.
    #[test]
    fn validate_splits_the_two_mobile_claims() {
        let panel = Fingerprint {
            seed: 7,
            panel_width: 1470,
            panel_height: 956,
            device_pixel_ratio: 2.0,
            ..Default::default()
        };
        assert!(
            validate(&panel, ProfileOs::Ios).is_err(),
            "ios accepted a display block it can never present"
        );
        // The seed alone is fine on the same profile — the refusal is about the display block,
        // not about the OS.
        let seed_only = Fingerprint {
            seed: 7,
            ..Default::default()
        };
        assert!(validate(&seed_only, ProfileOs::Ios).is_ok());
        // The desktop control: the identical fingerprint is a valid Windows profile.
        assert!(validate(&panel, ProfileOs::Windows).is_ok());

        // Android requires the phone block: no panel, no scale factor or no touch count each
        // refuse the save, and a landscape panel is a monitor wearing a phone's identity.
        let phone = Fingerprint {
            seed: 424242,
            panel_width: 412,
            panel_height: 892,
            device_pixel_ratio: 2.62,
            max_touch_points: Some(5),
            ..Default::default()
        };
        assert!(validate(&phone, ProfileOs::Android).is_ok());
        for incomplete in [
            Fingerprint {
                seed: 424242,
                ..Default::default()
            },
            Fingerprint {
                seed: 424242,
                panel_width: 412,
                panel_height: 892,
                device_pixel_ratio: 2.62,
                ..Default::default()
            },
            Fingerprint {
                seed: 424242,
                panel_width: 412,
                panel_height: 892,
                max_touch_points: Some(5),
                ..Default::default()
            },
            Fingerprint {
                seed: 424242,
                panel_width: 892,
                panel_height: 412,
                device_pixel_ratio: 2.62,
                max_touch_points: Some(5),
                ..Default::default()
            },
            Fingerprint {
                seed: 424242,
                panel_width: 412,
                panel_height: 892,
                device_pixel_ratio: 2.62,
                max_touch_points: Some(0),
                ..Default::default()
            },
        ] {
            assert!(
                validate(&incomplete, ProfileOs::Android).is_err(),
                "android accepted an incoherent phone: {incomplete:?}"
            );
        }
        // A seedless android profile claims nothing the engine must answer — the platform switch
        // needs the seed — so the empty form is still a valid (honest) profile.
        assert!(validate(&Fingerprint::default(), ProfileOs::Android).is_ok());
    }

    /// One identity source at a time. A literal `user_agent` reaches only the UA string, and
    /// measured against the engine it also *blanks* UA-CH's `platformVersion` and `architecture` —
    /// values no real browser returns empty. See 5.5b.
    #[test]
    fn a_literal_user_agent_and_a_seed_cannot_coexist() {
        let both = Fingerprint {
            seed: 7,
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)".into(),
            ..Default::default()
        };
        assert!(
            valid(&both).is_err(),
            "two identity sources must be rejected"
        );
        // Either one alone is fine: a literal UA is the only thing a stock engine can present, and a
        // seed is how a patched engine derives the whole identity.
        assert!(valid(&Fingerprint {
            seed: 0,
            ..both.clone()
        })
        .is_ok());
        assert!(valid(&Fingerprint {
            user_agent: String::new(),
            ..both
        })
        .is_ok());
    }

    #[test]
    fn webrtc_policy_maps_to_the_documented_upstream_values() {
        // `--webrtc-ip-handling-policy`, not `--force-…`: the latter is a `content/` switch that only
        // `content_shell` and headless read, so the `chrome` binary ignored it (6.7). `Disabled`
        // shares `ProxyOnly`'s value because Chromium has no switch that disables WebRTC.
        let cases = [
            (WebrtcPolicy::Default, None),
            (
                WebrtcPolicy::ProxyOnly,
                Some("--webrtc-ip-handling-policy=disable_non_proxied_udp"),
            ),
            (
                WebrtcPolicy::Disabled,
                Some("--webrtc-ip-handling-policy=disable_non_proxied_udp"),
            ),
        ];
        for (policy, expected) in cases {
            let fp = Fingerprint {
                webrtc_policy: policy,
                ..Default::default()
            };
            for dialect in [FlagDialect::StockChromium, FlagDialect::VeilPatched] {
                let args = opts(&fp, dialect).args;
                match expected {
                    Some(switch) => assert_eq!(args, vec![switch], "{policy:?} / {dialect:?}"),
                    None => assert!(args.is_empty(), "{policy:?} / {dialect:?} emitted {args:?}"),
                }
            }
        }
    }

    #[test]
    fn stock_chromium_is_the_default_dialect() {
        // Absent information means "assume no capability".
        assert_eq!(FlagDialect::default(), FlagDialect::StockChromium);
    }

    #[test]
    fn validate_accepts_default_and_consistent() {
        assert!(valid(&Fingerprint::default()).is_ok());
        let fp = Fingerprint {
            locale: "en-US".into(),
            languages: vec!["en-US".into()],
            hardware_concurrency: 8,
            device_memory: 8,
            ..Default::default()
        };
        assert!(valid(&fp).is_ok());
    }

    #[test]
    fn validate_rejects_inconsistent() {
        let bad_lang = Fingerprint {
            locale: "en-US".into(),
            languages: vec!["fr-FR".into()],
            ..Default::default()
        };
        assert!(valid(&bad_lang).is_err());
        let bad_screen = Fingerprint {
            screen_width: 1920,
            ..Default::default()
        };
        assert!(valid(&bad_screen).is_err());
        let bad_cores = Fingerprint {
            hardware_concurrency: 200,
            ..Default::default()
        };
        assert!(valid(&bad_cores).is_err());
    }

    /// Tier 1 `018-timezone` hands this string to ICU. Every accepted value here is one the engine
    /// will actually try to resolve, so the shapes that would break the clock must be rejected at
    /// configuration time rather than at launch.
    #[test]
    fn timezone_must_look_like_an_iana_zone_id() {
        for good in [
            "",
            "UTC",
            "GMT",
            "America/New_York",
            "Asia/Shanghai",
            "Europe/Isle_of_Man",
            "America/Argentina/Buenos_Aires",
            "Etc/GMT+8",
        ] {
            let fp = Fingerprint {
                timezone: good.into(),
                ..Default::default()
            };
            assert!(valid(&fp).is_ok(), "should accept {good:?}");
        }

        for bad in [
            "EST5EDT",                    // POSIX form — what Stage 1.6 measured being ignored
            "China Standard Time",        // Windows display name
            "America/",                   // empty component
            "/New_York",                  // empty area
            "america/new_york/extra/one", // too deep
            "America\\New_York",          // path separator, not a zone separator
            "America/New York",           // whitespace
            "12/34",                      // numeric area
            "NewYork",                    // no area at all, and not one of the two bare ids
        ] {
            let fp = Fingerprint {
                timezone: bad.into(),
                ..Default::default()
            };
            assert!(valid(&fp).is_err(), "should reject {bad:?}");
        }
    }

    /// Tier 1 derives canvas and audio noise from `--fingerprint=<seed>` alone: there is no
    /// `--canvas-noise` or `--audio-noise` switch anywhere in the patch set. Asking for noise
    /// without a seed therefore buys nothing, and a stored profile that claims it would make the
    /// UI report protection the engine does not provide.
    #[test]
    fn noise_requires_a_seed() {
        for noisy in [
            Fingerprint {
                canvas_noise: NoiseMode::Low,
                ..Default::default()
            },
            Fingerprint {
                audio_noise: NoiseMode::High,
                ..Default::default()
            },
        ] {
            assert!(valid(&noisy).is_err(), "seedless noise must be rejected");
            let seeded = Fingerprint { seed: 7, ..noisy };
            assert!(valid(&seeded).is_ok(), "seeded noise is coherent");
        }
    }

    /// `960` reads the panel, and `000` had declared the switches for five stages before it did. The
    /// gate is the seed for the same reason `--fingerprint-platform`'s is: the engine derives the work
    /// area, the colour depth and the orientation from the platform claim, and without a seed no
    /// platform claim is emitted.
    #[test]
    fn panel_switches_need_a_seed_and_the_patched_dialect() {
        let panel = Fingerprint {
            panel_width: 1470,
            panel_height: 956,
            device_pixel_ratio: 2.0,
            ..Default::default()
        };
        // Stock has none of the three, seedless has no platform to derive a work area from.
        assert!(opts(&panel, FlagDialect::StockChromium).args.is_empty());
        assert!(opts(&panel, FlagDialect::VeilPatched).args.is_empty());
        let seeded = Fingerprint { seed: 7, ..panel };
        assert_eq!(
            opts(&seeded, FlagDialect::VeilPatched).args,
            vec![
                "--fingerprint=7",
                "--fingerprint-platform=windows",
                "--fingerprint-screen-width=1470",
                "--fingerprint-screen-height=956",
                "--fingerprint-device-scale-factor=2.00",
                "--disable-spoofing=audio,canvas",
            ]
        );
    }

    /// One profile, one byte-identical command line. `1.5` and `1.50` are the same display and must
    /// not produce two argv, because a snapshot diff would read that as a launch change.
    #[test]
    fn scale_factor_is_pinned_to_two_decimals() {
        for (ratio, expected) in [
            (1.0, "--fingerprint-device-scale-factor=1.00"),
            (1.25, "--fingerprint-device-scale-factor=1.25"),
            (1.5, "--fingerprint-device-scale-factor=1.50"),
            (2.0, "--fingerprint-device-scale-factor=2.00"),
            (3.0, "--fingerprint-device-scale-factor=3.00"),
        ] {
            let fp = Fingerprint {
                seed: 7,
                device_pixel_ratio: ratio,
                ..Default::default()
            };
            let args = opts(&fp, FlagDialect::VeilPatched).args;
            assert!(args.contains(&expected.to_string()), "{ratio} -> {args:?}");
        }
        // A panel is not required to move the ratio, and a ratio is not required to move the panel.
        let ratio_only = Fingerprint {
            seed: 7,
            device_pixel_ratio: 2.0,
            ..Default::default()
        };
        let args = opts(&ratio_only, FlagDialect::VeilPatched).args;
        assert!(!args.iter().any(|a| a.starts_with("--fingerprint-screen-")));
    }

    /// The rule that needs no declared panel and no spec sheet: `002` reports UA-CH
    /// `architecture: arm` for every macOS profile, and no Apple Silicon Mac has shipped a display
    /// below 2×. A 1470×956 @1× persona is the pair that made the `-1.1001` snapshot describe a
    /// machine that does not exist, and this is where it stops being saveable.
    #[test]
    fn an_apple_silicon_panel_must_be_retina() {
        let retina = Fingerprint {
            seed: 424_242,
            panel_width: 1470,
            panel_height: 956,
            device_pixel_ratio: 2.0,
            canvas_noise: NoiseMode::Low,
            audio_noise: NoiseMode::Low,
            ..Default::default()
        };
        assert!(validate(&retina, ProfileOs::Macos).is_ok());
        // The same panel at 1×, and the same panel with no ratio at all: both report the host's
        // scale factor beside an Apple Silicon claim.
        for ratio in [1.0, 1.5, 0.0] {
            let fp = Fingerprint {
                device_pixel_ratio: ratio,
                ..retina.clone()
            };
            assert!(
                validate(&fp, ProfileOs::Macos).is_err(),
                "macOS at {ratio}× must be rejected"
            );
            // Windows laptops at 1× and 1.5× are ordinary, so the rule is macOS-only. The same
            // 1470×956 panel is a real Windows machine at any of these ratios, and no window is
            // declared for the work-area rule to catch.
            assert!(
                validate(&fp, ProfileOs::Windows).is_ok(),
                "windows at {ratio}× is a real machine"
            );
        }
    }

    /// Assertion 6 R2 (`window-fits-the-work-area`) applied to the declaration instead of the
    /// capture. It scored as a floor while nothing drove `screen.*`; now that `960` does, the
    /// configuration that would fail it cannot be saved — which is also why `960` leaves
    /// `outerWidth` / `outerHeight` alone where its counterpart clamps them.
    #[test]
    fn a_window_must_fit_the_work_area_its_panel_leaves() {
        // Windows reserves 48 px at the bottom, so a 1920×1080 panel leaves 1920×1032.
        let base = Fingerprint {
            seed: 7,
            panel_width: 1920,
            panel_height: 1080,
            device_pixel_ratio: 1.0,
            ..Default::default()
        };
        assert_eq!(
            derived_work_area(&base, ProfileOs::Windows),
            Some((1920, 1032))
        );
        for (width, height, ok) in [
            (1920u32, 1032u32, true), // exactly maximised
            (1280, 800, true),        // comfortably inside
            (1920, 1080, false),      // the panel, not the work area: 48 px too tall
            (2560, 800, false),       // wider than the panel
        ] {
            let fp = Fingerprint {
                screen_width: width,
                screen_height: height,
                ..base.clone()
            };
            assert_eq!(
                validate(&fp, ProfileOs::Windows).is_ok(),
                ok,
                "{width}×{height} window in a 1920×1080 Windows panel"
            );
        }
        // macOS reserves its 25 px at the top instead, so the same window fits a shorter panel.
        let mac = Fingerprint {
            panel_width: 1470,
            panel_height: 956,
            device_pixel_ratio: 2.0,
            screen_width: 1470,
            screen_height: 931,
            ..base.clone()
        };
        assert_eq!(derived_work_area(&mac, ProfileOs::Macos), Some((1470, 931)));
        assert!(validate(&mac, ProfileOs::Macos).is_ok());
    }

    /// The bounds are bounds, not a table of resolutions: a panel with no room for a shell and a
    /// scale factor no desktop reports are rejected, and everything between them is the user's call.
    #[test]
    fn the_panel_and_the_scale_factor_have_plausible_bounds() {
        let base = Fingerprint {
            seed: 7,
            device_pixel_ratio: 1.0,
            ..Default::default()
        };
        for (width, height) in [(640u32, 480u32), (1470, 956), (3840, 2160)] {
            let fp = Fingerprint {
                panel_width: width,
                panel_height: height,
                ..base.clone()
            };
            assert!(valid(&fp).is_ok(), "{width}×{height} should be accepted");
        }
        for (width, height) in [(320u32, 240u32), (1470, 0), (0, 956), (20000, 20000)] {
            let fp = Fingerprint {
                panel_width: width,
                panel_height: height,
                ..base.clone()
            };
            assert!(valid(&fp).is_err(), "{width}×{height} should be rejected");
        }
        for ratio in [0.5, 0.99, 4.01, 8.0] {
            let fp = Fingerprint {
                device_pixel_ratio: ratio,
                ..base.clone()
            };
            assert!(valid(&fp).is_err(), "{ratio}× should be rejected");
        }
    }

    /// The Veil half of the pair `960` carries in `display_persona.h`. Pinned here so a change to
    /// either side without the other shows up as a failing test rather than as a work area the
    /// engine and the validator disagree about.
    #[test]
    fn work_area_inset_matches_the_engine_table() {
        assert_eq!(work_area_inset(ProfileOs::Windows), (0, 48));
        assert_eq!(work_area_inset(ProfileOs::Macos), (25, 0));
        assert_eq!(work_area_inset(ProfileOs::Linux), (27, 0));
        // T24-b: a phone reports the work area equal to the full screen (990's display arm);
        // ios keeps the Windows strip because its claim never reaches 960 at all.
        assert_eq!(work_area_inset(ProfileOs::Android), (0, 0));
        assert_eq!(work_area_inset(ProfileOs::Ios), (0, 48));
    }
}
