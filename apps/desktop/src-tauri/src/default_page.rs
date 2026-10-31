use std::fs;
use std::path::Path;

use veil_core::{AppError, AppResult, ErrorCode};

const PAGE_DIR: &str = "default-page";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const BACKGROUNDS: &[(&str, &[u8])] = &[
    (
        "bg-01.jpg",
        include_bytes!("../default_page/backgrounds/bg-01.jpg"),
    ),
    (
        "bg-02.jpg",
        include_bytes!("../default_page/backgrounds/bg-02.jpg"),
    ),
    (
        "bg-03.jpg",
        include_bytes!("../default_page/backgrounds/bg-03.jpg"),
    ),
    (
        "bg-04.jpg",
        include_bytes!("../default_page/backgrounds/bg-04.jpg"),
    ),
    (
        "bg-05.jpg",
        include_bytes!("../default_page/backgrounds/bg-05.jpg"),
    ),
    (
        "bg-06.jpg",
        include_bytes!("../default_page/backgrounds/bg-06.jpg"),
    ),
    (
        "bg-07.jpg",
        include_bytes!("../default_page/backgrounds/bg-07.jpg"),
    ),
    (
        "bg-08.jpg",
        include_bytes!("../default_page/backgrounds/bg-08.jpg"),
    ),
    (
        "bg-09.jpg",
        include_bytes!("../default_page/backgrounds/bg-09.jpg"),
    ),
    (
        "bg-10.jpg",
        include_bytes!("../default_page/backgrounds/bg-10.jpg"),
    ),
    (
        "bg-11.jpg",
        include_bytes!("../default_page/backgrounds/bg-11.jpg"),
    ),
    (
        "bg-12.jpg",
        include_bytes!("../default_page/backgrounds/bg-12.jpg"),
    ),
    (
        "bg-13.jpg",
        include_bytes!("../default_page/backgrounds/bg-13.jpg"),
    ),
    (
        "bg-14.jpg",
        include_bytes!("../default_page/backgrounds/bg-14.jpg"),
    ),
    (
        "bg-15.jpg",
        include_bytes!("../default_page/backgrounds/bg-15.jpg"),
    ),
    (
        "bg-16.jpg",
        include_bytes!("../default_page/backgrounds/bg-16.jpg"),
    ),
    (
        "bg-17.jpg",
        include_bytes!("../default_page/backgrounds/bg-17.jpg"),
    ),
    (
        "bg-18.jpg",
        include_bytes!("../default_page/backgrounds/bg-18.jpg"),
    ),
    (
        "bg-19.jpg",
        include_bytes!("../default_page/backgrounds/bg-19.jpg"),
    ),
    (
        "bg-20.jpg",
        include_bytes!("../default_page/backgrounds/bg-20.jpg"),
    ),
    (
        "bg-21.jpg",
        include_bytes!("../default_page/backgrounds/bg-21.jpg"),
    ),
];

pub fn resolve_startup_url(
    data_dir: &Path,
    startup_page: &str,
    startup_url: &str,
    engine_version: &str,
) -> AppResult<String> {
    match startup_page {
        "google" => Ok("https://www.google.com/".to_string()),
        "bing" => Ok("https://www.bing.com/".to_string()),
        "custom" => Ok(normalize_custom_url(startup_url)),
        _ => local_default_page_url(data_dir, engine_version),
    }
}

fn local_default_page_url(data_dir: &Path, engine_version: &str) -> AppResult<String> {
    let page_root = data_dir.join(PAGE_DIR);
    let backgrounds_dir = page_root.join("backgrounds");
    fs::create_dir_all(&backgrounds_dir)
        .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;

    fs::write(page_root.join("index.html"), default_page_html())
        .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
    for (name, bytes) in BACKGROUNDS {
        let target = backgrounds_dir.join(name);
        if !target.exists() {
            fs::write(target, bytes)
                .map_err(|_| AppError::from(ErrorCode::StorageDatabaseWriteFailed))?;
        }
    }

    let selected = random_background_name();
    Ok(format!(
        "{}?bg={}&app={}&engine={}",
        file_url(&page_root.join("index.html")),
        selected,
        url_component(APP_VERSION),
        url_component(engine_version),
    ))
}

fn random_background_name() -> &'static str {
    let nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let idx = nanos.unsigned_abs() as usize % BACKGROUNDS.len();
    BACKGROUNDS[idx].0
}

fn normalize_custom_url(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "about:blank".to_string();
    }
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("file://")
        || trimmed.starts_with("about:")
    {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

fn file_url(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    format!("file:///{}", raw.replace(' ', "%20"))
}

fn url_component(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn default_page_html() -> &'static str {
    r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Veil</title>
  <style>
    :root { color-scheme: dark; font-family: Inter, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
    * { box-sizing: border-box; }
    body {
      min-height: 100vh;
      margin: 0;
      display: grid;
      place-items: center;
      background: #101010 center / cover no-repeat fixed;
      color: #fff;
      overflow: hidden;
    }
    body::before {
      content: "";
      position: fixed;
      inset: 0;
      background:
        linear-gradient(180deg, rgba(0,0,0,.42), rgba(0,0,0,.64)),
        radial-gradient(70% 50% at 50% 30%, rgba(126,109,221,.18), transparent 65%);
      pointer-events: none;
    }
    main {
      position: relative;
      width: min(760px, calc(100vw - 48px));
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 18px;
      text-align: center;
    }
    .brand { font-size: 46px; font-weight: 650; letter-spacing: 0; text-shadow: 0 16px 40px rgba(0,0,0,.45); }
    .search {
      width: 100%;
      display: grid;
      grid-template-columns: 1fr 132px;
      gap: 8px;
      padding: 8px;
      border: 1px solid rgba(255,255,255,.2);
      border-radius: 12px;
      background: rgba(24,24,24,.72);
      box-shadow: 0 18px 70px rgba(0,0,0,.36);
      backdrop-filter: blur(20px) saturate(140%);
    }
    input, select {
      height: 48px;
      border: 0;
      border-radius: 8px;
      padding: 0 16px;
      background: rgba(255,255,255,.1);
      color: #fff;
      font: inherit;
      outline: none;
    }
    select { cursor: pointer; }
    option { color: #111; }
    footer {
      position: fixed;
      left: 22px;
      right: 22px;
      bottom: 16px;
      display: flex;
      justify-content: space-between;
      gap: 16px;
      color: rgba(255,255,255,.66);
      font-size: 12px;
      font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
      text-shadow: 0 8px 20px rgba(0,0,0,.55);
    }
  </style>
</head>
<body>
  <main>
    <div class="brand">Veil</div>
    <form class="search" id="searchForm">
      <input id="query" autocomplete="off" spellcheck="false" placeholder="Search or enter address" />
      <select id="engine" aria-label="Search engine">
        <option value="google">Google</option>
        <option value="bing">Bing</option>
      </select>
    </form>
  </main>
  <footer>
    <span>Veil <span id="appVersion"></span></span>
    <span>Engine <span id="engineVersion"></span></span>
  </footer>
  <script>
    const params = new URLSearchParams(location.search);
    const bg = params.get("bg") || "bg-01.jpg";
    document.body.style.backgroundImage = `url("./backgrounds/${bg}")`;
    document.getElementById("appVersion").textContent = params.get("app") || "0.0.0";
    document.getElementById("engineVersion").textContent = params.get("engine") || "unknown";

    const engines = {
      google: "https://www.google.com/search?q=",
      bing: "https://www.bing.com/search?q="
    };
    document.getElementById("searchForm").addEventListener("submit", event => {
      event.preventDefault();
      const value = document.getElementById("query").value.trim();
      if (!value) return;
      if (/^(https?:\/\/|file:\/\/|about:)/i.test(value)) location.href = value;
      else if (value.includes(".") && !value.includes(" ")) location.href = `https://${value}`;
      else location.href = engines[document.getElementById("engine").value] + encodeURIComponent(value);
    });
  </script>
</body>
</html>"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_custom_urls() {
        assert_eq!(normalize_custom_url("example.com"), "https://example.com");
        assert_eq!(
            normalize_custom_url("https://example.com"),
            "https://example.com"
        );
        assert_eq!(normalize_custom_url(" "), "about:blank");
    }

    #[test]
    fn resolves_known_startup_pages() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            resolve_startup_url(dir.path(), "google", "", "146").unwrap(),
            "https://www.google.com/"
        );
        assert_eq!(
            resolve_startup_url(dir.path(), "bing", "", "146").unwrap(),
            "https://www.bing.com/"
        );
        assert_eq!(
            resolve_startup_url(dir.path(), "custom", "example.com", "146").unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn local_default_page_writes_html_and_backgrounds() {
        let dir = tempfile::tempdir().expect("tempdir");
        let url = resolve_startup_url(dir.path(), "veil", "", "146.0").unwrap();
        assert!(url.starts_with("file:///"));
        assert!(url.contains("engine=146.0"));
        let page = dir.path().join(PAGE_DIR).join("index.html");
        assert!(page.exists());
        assert!(dir
            .path()
            .join(PAGE_DIR)
            .join("backgrounds")
            .join("bg-01.jpg")
            .exists());

        // The bytes that land on disk are the page the engine actually loads, so the no-network
        // guarantee is asserted on the written file too, not only on the in-memory constant.
        let written = fs::read_to_string(&page).expect("read page");
        assert_eq!(written, default_page_html());
        assert!(!written.contains("ipinfo"));
    }

    /// The start page is a local file and nothing the user did is in flight when it loads, so it
    /// must not reach the network at all. It used to `fetch` `api.ipinfo.io` unconditionally on
    /// load and print the exit IP, country and ASN — a background call to a third party, made from
    /// inside the engine page and bypassing `crates/runtime` entirely. That broke README
    /// ("Veil never contacts the network in the background") and the security model's rule that
    /// "network fetches are user-initiated and routed through `crates/runtime`" at the same time.
    #[test]
    fn local_default_page_makes_no_network_request() {
        let html = default_page_html();

        assert!(
            !html.contains("ipinfo"),
            "the start page must not reference ipinfo"
        );
        for primitive in [
            "fetch(",
            "XMLHttpRequest",
            "sendBeacon",
            "WebSocket",
            "EventSource",
        ] {
            assert!(
                !html.contains(primitive),
                "the start page must contain no network primitive, found `{primitive}`"
            );
        }
    }

    /// Deleting the background fetch must not take the user's own navigation with it: submitting
    /// the form still hands the query to Google/Bing, and typing a URL still navigates to it.
    #[test]
    fn local_default_page_keeps_the_user_initiated_search_form() {
        let html = default_page_html();

        assert!(html.contains(r#"id="searchForm""#));
        assert!(html.contains("https://www.google.com/search?q="));
        assert!(html.contains("https://www.bing.com/search?q="));
        assert!(html.contains("location.href = value"));
        assert!(html.contains(r#"location.href = `https://${value}`"#));
    }

    #[test]
    fn local_default_page_drops_the_ip_readout() {
        let html = default_page_html();

        assert!(!html.contains(r#"id="ip""#));
        assert!(!html.contains(r#"class="ip""#));
        assert!(!html.contains(".ip {"));
        assert!(!html.contains("Current IP"));
    }
}
