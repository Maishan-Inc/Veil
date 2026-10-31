//! Randomized fingerprint presets (Phase 3).
//!
//! Randomization lives in the backend (not the View) and is deterministic given a seed so it
//! can be unit-tested. The pool values are coherent — picking one OS yields a matching UA,
//! WebGL vendor/renderer, locale and language list — so the result always passes
//! `veil_core::validate`. No fingerprint logic lives here; we only assemble plausible config.

use veil_core::{Fingerprint, NoiseMode, ProfileOs, WebrtcPolicy};

use crate::ProxyTestOutcome;

/// Tiny dependency-free PRNG (xorshift64). Deterministic so tests can assert exact output;
/// we only need spread, not cryptographic quality.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn pick<'a, T>(&mut self, pool: &'a [T]) -> &'a T {
        &pool[(self.next() % pool.len() as u64) as usize]
    }
}

/// Declared display panels: `(width, height, devicePixelRatio)` in DIPs, the unit `screen.width`
/// reports. Split in two pools because `002` reports UA-CH `architecture: arm` for **every**
/// `--fingerprint-platform=macos` profile, so a macOS preset is an Apple Silicon claim and no Apple
/// Silicon Mac has shipped a display below 2× — a non-Retina macOS preset is a profile
/// `veil_core::validate` refuses to save (`apple-silicon-implies-retina`).
///
/// These are declarations of intent, not a hardware catalogue: the point is that the pair is
/// self-consistent and that the engine can be caught reporting something else.
const PANELS_RETINA: [(u32, u32, f64); 3] = [(1470, 956, 2.0), (1512, 982, 2.0), (1728, 1117, 2.0)];
const PANELS_STANDARD: [(u32, u32, f64); 4] = [
    (1920, 1080, 1.0),
    (1536, 864, 1.0),
    (1366, 768, 1.0),
    (2560, 1440, 1.0),
];
/// How much of the work area the window leaves free. `(0, 0)` is a maximised window, which is both
/// the commonest real shape and the one that makes `window-fits-the-work-area` exact.
const WINDOW_MARGINS: [(u32, u32); 3] = [(0, 0), (80, 60), (160, 120)];
const TIMEZONES: [&str; 3] = ["America/New_York", "Europe/London", "Asia/Tokyo"];
const CORES: [u8; 4] = [4, 8, 12, 16];
const MEMORY: [u8; 3] = [8, 16, 32];

/// Produce a coherent random fingerprint for the given OS. Same seed → same result.
pub fn randomize(os: &str, seed: u64) -> Fingerprint {
    let mut rng = Rng::new(seed);
    let profile_os = ProfileOs::from_str_or_default(os);
    let declared = seed != 0;
    // `seed == 0` means "let the engine randomize each launch", and the patch set derives canvas and
    // audio noise from `--fingerprint=<seed>` alone — with no seed there is no noise. Claiming a level
    // anyway is what `veil_core::validate` rejects, so the preset must not produce it.
    let noise = if declared {
        NoiseMode::Low
    } else {
        NoiseMode::Off
    };

    // T24-b (6.26): the android claim is a phone, and the phone is the ROW the seed picks — the
    // same row the engine's `990` reads, from the one mirrored table (`veil_core::android_device`).
    // Window, panel, DPR, touch count, cores, memory and the GPU strings are one machine rather
    // than five guesses; the window is the row's panel minus the phone browser chrome, which is
    // 6.24's launcher coupling done on the management side. Seedless keeps the honest shape: a
    // window and nothing else, exactly like the desktop arm below.
    if profile_os == ProfileOs::Android {
        let device = veil_core::android_device::device_for_seed(seed as u32);
        let (window_width, window_height) = veil_core::android_device::phone_window(device);
        return Fingerprint {
            seed,
            user_agent: String::new(),
            screen_width: window_width,
            screen_height: window_height,
            panel_width: if declared { device.panel_width } else { 0 },
            panel_height: if declared { device.panel_height } else { 0 },
            device_pixel_ratio: if declared {
                device.device_pixel_ratio
            } else {
                0.0
            },
            timezone: rng.pick(&TIMEZONES).to_string(),
            locale: "en-US".to_string(),
            languages: vec!["en-US".to_string(), "en".to_string()],
            webgl_vendor: device.gl_vendor.to_string(),
            webgl_renderer: device.gl_renderer.to_string(),
            canvas_noise: noise,
            audio_noise: noise,
            fonts: vec!["Arial".to_string(), "Helvetica".to_string()],
            hardware_concurrency: device.hardware_concurrency,
            device_memory: device.device_memory_gb,
            max_touch_points: declared.then_some(device.max_touch_points),
            location: String::new(),
            webrtc_policy: WebrtcPolicy::ProxyOnly,
        };
    }

    let (vendor, renderer) = gpu_profile(os);
    let pool: &[(u32, u32, f64)] = if profile_os == ProfileOs::Macos {
        &PANELS_RETINA
    } else {
        &PANELS_STANDARD
    };
    let (panel_width, panel_height, device_pixel_ratio) = *rng.pick(pool);
    let (margin_x, margin_y) = *rng.pick(&WINDOW_MARGINS);
    // The window is sized against the work area the panel leaves, using the same inset table
    // `veil_core::validate` checks it against and `960` derives `screen.availHeight` from. Sizing it
    // against the *panel* is how a preset would produce a window hanging below its own taskbar.
    let (inset_top, inset_bottom) = veil_core::work_area_inset(profile_os);
    let work_height = panel_height.saturating_sub(inset_top + inset_bottom);
    // A declared panel and a scale factor reach the engine only alongside a seed — `960` derives the
    // work area from `--fingerprint-platform`, which `to_engine_args` emits only with one — so a
    // seedless preset declares no display and keeps only the window it always had.
    let declared = seed != 0;
    // The display block has a second gate, and it is the profile's OS: a platform-less OS gets no
    // `--fingerprint-platform` at all, so `validate` refuses a panel on one and `to_engine_args`
    // emits none of it (T24). The preset asks the same
    // predicate the two of them read rather than repeating the OS list, so a preset can never be a
    // profile that the very next save rejects — which is what this test has always asserted.
    let declares_display = declared && profile_os.has_fingerprint_platform();
    Fingerprint {
        seed,
        // Leave UA empty so the launched engine exposes its real version by default.
        // Users can still enter a custom UA in the profile dialog.
        user_agent: String::new(),
        screen_width: panel_width.saturating_sub(margin_x),
        screen_height: work_height.saturating_sub(margin_y),
        panel_width: if declares_display { panel_width } else { 0 },
        panel_height: if declares_display { panel_height } else { 0 },
        device_pixel_ratio: if declares_display {
            device_pixel_ratio
        } else {
            0.0
        },
        timezone: rng.pick(&TIMEZONES).to_string(),
        locale: "en-US".to_string(),
        languages: vec!["en-US".to_string(), "en".to_string()],
        webgl_vendor: vendor.to_string(),
        webgl_renderer: renderer.to_string(),
        canvas_noise: noise,
        audio_noise: noise,
        fonts: vec!["Arial".to_string(), "Helvetica".to_string()],
        hardware_concurrency: *rng.pick(&CORES),
        device_memory: *rng.pick(&MEMORY),
        // Not randomized, and 0 is a declaration rather than a fallback. A Mac reports 0 touch points
        // structurally — macOS builds `pointer_device_default.cc`, whose `MaxTouchPoints()` is a
        // constant — so `veil_core::validate` refuses anything else there; and on Windows and Linux the
        // overwhelming majority of desktops report 0 too, so it is the modal claim rather than a
        // guess. A user who wants a touch laptop types the count. Picking from a pool here would be a
        // digitizer catalogue, which is the kind of table this crate keeps out of presets.
        //
        // Seedless presets declare nothing, exactly as they declare no display: "let the engine be
        // itself" is the whole meaning of `seed == 0`, and `961` honours this switch without a seed, so
        // sending it anyway would spoof a surface a seedless profile promised to leave alone.
        max_touch_points: if declared { Some(0) } else { None },
        // Deliberately empty, and the one geography answer the preset refuses to guess: Veil is
        // offline and holds no geo-IP table, so a random coordinate would put the persona in an
        // ocean or a field beside the timezone it also declares -- a contradiction a page scores
        // in one read. The coordinates are the user's to point at (`veil_core::validate` enforces
        // the timezone pairing), and `970` answers nothing while the field is empty.
        location: String::new(),
        webrtc_policy: WebrtcPolicy::ProxyOnly,
    }
}

/// Produce a coherent fingerprint using a user-initiated proxy probe as the source
/// of truth for geography. The proxy's timezone wins when present; locale/languages
/// come from the detected country code.
pub fn randomize_for_proxy(os: &str, seed: u64, proxy: &ProxyTestOutcome) -> Fingerprint {
    let mut fp = randomize(os, seed);
    let (locale, languages, fallback_timezone) = locale_profile(proxy.country_code.as_deref());
    fp.locale = locale.to_string();
    fp.languages = languages.iter().map(|value| value.to_string()).collect();
    fp.timezone = proxy
        .timezone
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback_timezone.to_string());
    fp
}

fn gpu_profile(os: &str) -> (&'static str, &'static str) {
    match os {
        "macos" | "ios" => ("Apple Inc.", "Apple GPU"),
        "android" => ("Qualcomm", "Adreno (TM) 730"),
        "linux" => ("Mesa", "llvmpipe"),
        _ => (
            "Google Inc. (Intel)",
            "ANGLE (Intel, Intel(R) UHD Graphics)",
        ),
    }
}

fn locale_profile(
    country_code: Option<&str>,
) -> (&'static str, &'static [&'static str], &'static str) {
    match country_code
        .unwrap_or_default()
        .to_ascii_uppercase()
        .as_str()
    {
        "CN" => ("zh-CN", &["zh-CN", "zh", "en-US", "en"], "Asia/Shanghai"),
        "HK" => ("zh-HK", &["zh-HK", "zh-CN", "zh", "en"], "Asia/Hong_Kong"),
        "TW" => ("zh-TW", &["zh-TW", "zh", "en-US", "en"], "Asia/Taipei"),
        "JP" => ("ja-JP", &["ja-JP", "ja", "en-US", "en"], "Asia/Tokyo"),
        "KR" => ("ko-KR", &["ko-KR", "ko", "en-US", "en"], "Asia/Seoul"),
        "SG" => ("en-SG", &["en-SG", "en", "zh-CN", "zh"], "Asia/Singapore"),
        "US" => ("en-US", &["en-US", "en"], "America/New_York"),
        "GB" => ("en-GB", &["en-GB", "en"], "Europe/London"),
        "CA" => ("en-CA", &["en-CA", "en-US", "en"], "America/Toronto"),
        "AU" => ("en-AU", &["en-AU", "en"], "Australia/Sydney"),
        "DE" => ("de-DE", &["de-DE", "de", "en-US", "en"], "Europe/Berlin"),
        "FR" => ("fr-FR", &["fr-FR", "fr", "en-US", "en"], "Europe/Paris"),
        "NL" => ("nl-NL", &["nl-NL", "nl", "en-US", "en"], "Europe/Amsterdam"),
        "BR" => (
            "pt-BR",
            &["pt-BR", "pt", "en-US", "en"],
            "America/Sao_Paulo",
        ),
        "IN" => ("en-IN", &["en-IN", "en", "hi-IN", "hi"], "Asia/Kolkata"),
        "RU" => ("ru-RU", &["ru-RU", "ru", "en-US", "en"], "Europe/Moscow"),
        _ => ("en-US", &["en-US", "en"], "America/New_York"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_same_seed() {
        assert_eq!(randomize("windows", 42), randomize("windows", 42));
    }

    #[test]
    fn randomized_is_always_valid() {
        for os in ["windows", "macos", "linux", "android", "ios"] {
            // `0` is in range: the UI draws the seed from `Math.random()`, so it is reachable, and a
            // seedless preset must still satisfy `validate` — see the noise/seed coherence rule.
            for seed in 0..50 {
                assert!(veil_core::validate(
                    &randomize(os, seed),
                    veil_core::ProfileOs::from_str_or_default(os)
                )
                .is_ok());
            }
        }
        assert_eq!(randomize("windows", 0).canvas_noise, NoiseMode::Off);
    }

    /// T24-b's preset half (6.26): the two mobile claims split the same way they do in
    /// `validate`. An **android** preset IS the phone the seed picks — panel, DPR, touch count,
    /// cores and memory all from one mirrored device row, the same row the engine's `990` reads,
    /// with the window derived from the row (6.24's launcher coupling). An **ios** preset still
    /// declares no display block, because the value reaches nothing. A seedless preset on either
    /// declares a window and nothing else.
    #[test]
    fn mobile_presets_follow_their_claims() {
        // Android: the row is the seed's, and the whole machine comes with it.
        let android = randomize("android", 424242);
        let device = veil_core::android_device::device_for_seed(424242);
        assert_eq!(
            (android.panel_width, android.panel_height),
            (device.panel_width, device.panel_height)
        );
        assert_eq!(android.device_pixel_ratio, device.device_pixel_ratio);
        assert_eq!(android.max_touch_points, Some(device.max_touch_points));
        assert_eq!(android.hardware_concurrency, device.hardware_concurrency);
        assert_eq!(android.device_memory, device.device_memory_gb);
        assert_eq!(android.webgl_renderer, device.gl_renderer);
        // The window fits the panel: one machine, not a panel from the row beside a window from
        // the host.
        assert!(android.screen_width <= android.panel_width);
        assert!(android.screen_height <= android.panel_height);
        assert!(veil_core::validate(&android, veil_core::ProfileOs::Android).is_ok());

        // iOS: no display block — the value reaches nothing — but the rest of the persona stands.
        let ios = randomize("ios", 7);
        assert_eq!(
            (ios.panel_width, ios.panel_height, ios.device_pixel_ratio),
            (0, 0, 0.0)
        );
        assert!(ios.screen_width > 0 && ios.screen_height > 0);
        assert_eq!(ios.seed, 7);
        assert_ne!(ios.canvas_noise, NoiseMode::Off);

        // Seedless android: a window and nothing else, exactly like the desktop arm.
        let seedless = randomize("android", 0);
        assert_eq!((seedless.panel_width, seedless.panel_height), (0, 0));
        assert_eq!(seedless.max_touch_points, None);
        assert!(seedless.screen_width > 0 && seedless.screen_height > 0);

        // The desktop control: the same seed on Windows still declares a panel and a scale factor.
        let windows = randomize("windows", 7);
        assert!(windows.panel_width > 0 && windows.device_pixel_ratio > 0.0);
    }

    #[test]
    fn os_picks_matching_webgl() {
        assert_eq!(randomize("macos", 7).webgl_vendor, "Apple Inc.");
        assert_eq!(randomize("windows", 7).webgl_vendor, "Google Inc. (Intel)");
    }

    #[test]
    fn randomize_leaves_user_agent_empty() {
        assert!(randomize("windows", 7).user_agent.is_empty());
    }

    #[test]
    fn proxy_optimized_fingerprint_uses_detected_region() {
        let proxy = ProxyTestOutcome {
            ip: Some("203.0.113.10".to_string()),
            country: Some("Japan".to_string()),
            country_code: Some("JP".to_string()),
            region: Some("Tokyo".to_string()),
            city: Some("Tokyo".to_string()),
            timezone: Some("Asia/Tokyo".to_string()),
            latency_ms: Some(50),
        };
        let fp = randomize_for_proxy("windows", 42, &proxy);
        assert_eq!(fp.locale, "ja-JP");
        assert_eq!(fp.languages[0], "ja-JP");
        assert_eq!(fp.timezone, "Asia/Tokyo");
        assert!(fp.user_agent.is_empty());
    }
}
