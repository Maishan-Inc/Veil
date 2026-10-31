//! The phone table — the management-side mirror of the engine's
//! `components/ungoogled/android_device_data.h` (`990`, T24-a).
//!
//! Two copies of one table is normally the bug this project keeps removing, and it is accepted
//! here for the same stated reason [`crate::fingerprint::work_area_inset`] is: the engine picks
//! the row from `--fingerprint=<seed>` at launch, and the launcher has to *predict* the same row
//! when it derives `--window-size` and validates the panel — the profile is validated long
//! before an engine exists to ask. 6.24 named this coupling ("the fork computes no viewport, so
//! the persona needs a phone-sized `--window-size` from the launcher — same device row, not the
//! host's"), and a second source for the row would be a second identity. The rows, weights and
//! mixing are byte-for-byte the engine header's, so a drift shows up as a profile whose window
//! disagrees with its own device.
//!
//! [`AndroidDevice`] carries the fields a profile needs: the portrait panel + DPR (what the
//! display block stores), the touch count (what `961`'s android rung answers) and the RAM/cores
//! the row claims (what the profile should declare beside it).

/// One phone row. Same rows, same order, same weights as the engine header — keep them in step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AndroidDevice {
    /// UA-CH model, e.g. `"Pixel 7"`.
    pub model: &'static str,
    /// UA-CH `platformVersion`, e.g. `"14.0.0"`.
    pub platform_version: &'static str,
    /// Portrait panel in CSS px — what `screen.width`/`screen.height` report.
    pub panel_width: u32,
    pub panel_height: u32,
    /// `devicePixelRatio`.
    pub device_pixel_ratio: f64,
    /// `navigator.maxTouchPoints` (`961`'s android rung answers this).
    pub max_touch_points: u8,
    /// `navigator.hardwareConcurrency` the row claims.
    pub hardware_concurrency: u8,
    /// `navigator.deviceMemory` the row claims (Chromium reports at most 8).
    pub device_memory_gb: u8,
    /// WebGL `UNMASKED_VENDOR_WEBGL` — what the engine's `011`/`990` android arm answers.
    pub gl_vendor: &'static str,
    /// WebGL `UNMASKED_RENDERER_WEBGL` (ANGLE-wrapped GLES).
    pub gl_renderer: &'static str,
}

/// Rows and weights are public hardware specifications, not measurements anyone has to take on
/// faith: Pixel 7 (Tensor G2, Mali-G710, 1080×2400 at 2.625× → 412×915 CSS), Galaxy S23
/// (SM-S911B, Adreno 740, 1080×2340 at 3.0× → 360×780 CSS), Galaxy A54 (SM-A546B, Exynos 1380,
/// Mali-G68 MC4, 1080×2340 at 2.625× → 412×892 CSS). All Android 14, all five-touch, all OLED
/// P3. Weights 50 / 30 / 20 — the flagships the population skews to, with the A54 the mid-tier
/// counterweight. Identical to `kAndroidDevices` in the engine header.
const DEVICES: [AndroidDevice; 3] = [
    AndroidDevice {
        model: "Pixel 7",
        platform_version: "14.0.0",
        panel_width: 412,
        panel_height: 915,
        device_pixel_ratio: 2.625,
        max_touch_points: 5,
        hardware_concurrency: 8,
        device_memory_gb: 8,
        gl_vendor: "Google Inc. (ARM)",
        gl_renderer: "ANGLE (ARM, Mali-G710, OpenGL ES 3.2)",
    },
    AndroidDevice {
        model: "SM-S911B",
        platform_version: "14.0.0",
        panel_width: 360,
        panel_height: 780,
        device_pixel_ratio: 3.0,
        max_touch_points: 5,
        hardware_concurrency: 8,
        device_memory_gb: 8,
        gl_vendor: "Google Inc. (Qualcomm)",
        gl_renderer: "ANGLE (Qualcomm, Adreno (TM) 740, OpenGL ES 3.2)",
    },
    AndroidDevice {
        model: "SM-A546B",
        platform_version: "14.0.0",
        panel_width: 412,
        panel_height: 892,
        device_pixel_ratio: 2.625,
        max_touch_points: 5,
        hardware_concurrency: 8,
        device_memory_gb: 8,
        gl_vendor: "Google Inc. (ARM)",
        gl_renderer: "ANGLE (ARM, Mali-G68 MC4, OpenGL ES 3.2)",
    },
];

/// Row weights out of 100, index-aligned with [`DEVICES`]: 50 / 30 / 20.
const WEIGHTS: [usize; 3] = [50, 30, 20];

/// splitmix64, the same mixing the engine header uses — weak seeds (1, 2, 3) otherwise land on
/// adjacent picks. Pure, so the launcher and the engine agree with no shared state.
const fn mix_seed(seed: u32) -> u64 {
    let mut mixed = seed as u64 + 0x9E37_79B9_7F4A_7C15;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

/// The row this seed presents — the same row the engine's `990` picks. Deterministic.
pub const fn device_for_seed(seed: u32) -> &'static AndroidDevice {
    let pick = (mix_seed(seed) % 100) as usize;
    let mut acc = 0usize;
    let mut index = 0usize;
    while index < DEVICES.len() {
        acc += WEIGHTS[index];
        if pick < acc {
            return &DEVICES[index];
        }
        index += 1;
    }
    &DEVICES[0]
}

/// The phone window a panel implies: the row's CSS height minus the browser chrome a
/// Chrome-on-Android window reserves (toolbar + system bar, ~112 CSS px — `412×780` on the A54's
/// `412×892`). This is the launcher half of 6.24's coupling: the window and the panel come from
/// ONE row, never one from the row and one from the host. Clamped so a tiny panel still keeps a
/// usable window.
pub fn phone_window(device: &AndroidDevice) -> (u32, u32) {
    let height = device
        .panel_height
        .saturating_sub(112)
        .max(device.panel_height / 2);
    (device.panel_width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_persona_seed_picks_the_a54_row() {
        // Seed 424242 is the harness persona's seed; the engine must pick the same row.
        let device = device_for_seed(424242);
        assert_eq!(device.model, "SM-A546B");
        assert_eq!(device.panel_width, 412);
        assert_eq!(device.panel_height, 892);
        assert_eq!(device.device_pixel_ratio, 2.625);
        assert_eq!(device.max_touch_points, 5);
    }

    #[test]
    fn weak_seeds_do_not_collapse_onto_one_row() {
        // The reason for the splitmix64: 1, 2 and 3 must not all land together.
        let rows: [&str; 3] = [
            device_for_seed(1).model,
            device_for_seed(2).model,
            device_for_seed(3).model,
        ];
        assert!(rows[0] != rows[1] || rows[1] != rows[2]);
    }

    #[test]
    fn the_window_fits_the_panel_and_carries_the_chrome() {
        let (width, height) = phone_window(device_for_seed(424242));
        assert_eq!(width, 412);
        assert_eq!(height, 780);
        let device = device_for_seed(424242);
        assert!(height < device.panel_height);
    }
}
