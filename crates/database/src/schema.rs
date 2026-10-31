use rusqlite::Connection;

/// Create tables if they do not exist. Idempotent; safe to call on every open.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS groups (
            id          TEXT PRIMARY KEY NOT NULL,
            name        TEXT NOT NULL,
            color       TEXT NULL,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS proxies (
            id          TEXT PRIMARY KEY NOT NULL,
            name        TEXT NOT NULL,
            kind        TEXT NOT NULL,
            host        TEXT NOT NULL,
            port        INTEGER NOT NULL,
            username    TEXT NULL,
            password    TEXT NULL,
            country     TEXT NULL,
            detected_ip TEXT NULL,
            country_code TEXT NULL,
            region      TEXT NULL,
            city        TEXT NULL,
            timezone    TEXT NULL,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_proxies_kind ON proxies(kind);

        CREATE TABLE IF NOT EXISTS profiles (
            id               TEXT PRIMARY KEY NOT NULL,
            name             TEXT NOT NULL,
            avatar_color     TEXT NOT NULL DEFAULT '#5640C9',
            group_id         TEXT NULL REFERENCES groups(id) ON DELETE SET NULL,
            tag_ids          TEXT NOT NULL DEFAULT '[]',
            proxy_id         TEXT NULL REFERENCES proxies(id) ON DELETE SET NULL,
            os               TEXT NOT NULL DEFAULT 'windows',
            platform         TEXT NOT NULL DEFAULT '',
            browser_version  TEXT NOT NULL DEFAULT '',
            notes            TEXT NOT NULL DEFAULT '',
            status           TEXT NOT NULL DEFAULT 'idle',
            startup_page     TEXT NOT NULL DEFAULT 'veil',
            startup_url      TEXT NOT NULL DEFAULT '',
            fingerprint      TEXT NOT NULL DEFAULT '{}',
            created_at       TEXT NOT NULL,
            updated_at       TEXT NOT NULL,
            last_launched_at TEXT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_profiles_group ON profiles(group_id);
        CREATE INDEX IF NOT EXISTS idx_profiles_proxy ON profiles(proxy_id);
        CREATE INDEX IF NOT EXISTS idx_profiles_updated ON profiles(updated_at);

        CREATE TABLE IF NOT EXISTS profile_cookie_jars (
            profile_id   TEXT PRIMARY KEY NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
            cookies_json TEXT NOT NULL DEFAULT '[]',
            updated_at   TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS profile_credentials (
            id         TEXT PRIMARY KEY NOT NULL,
            profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
            platform   TEXT NOT NULL,
            username   TEXT NOT NULL,
            password   TEXT NOT NULL,
            notes      TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_profile_credentials_profile ON profile_credentials(profile_id, updated_at);

        CREATE TABLE IF NOT EXISTS ai_settings (
            id                     TEXT PRIMARY KEY NOT NULL,
            provider_kind          TEXT NOT NULL,
            base_url               TEXT NOT NULL,
            model                  TEXT NOT NULL,
            reasoning_effort       TEXT NOT NULL,
            review_mode            TEXT NOT NULL,
            network_enabled        INTEGER NOT NULL DEFAULT 0,
            api_key                TEXT NULL,
            updated_at             TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS ai_model_configs (
            id                     TEXT PRIMARY KEY NOT NULL,
            name                   TEXT NOT NULL,
            provider_kind          TEXT NOT NULL,
            icon                   TEXT NOT NULL,
            base_url               TEXT NOT NULL,
            model                  TEXT NOT NULL,
            reasoning_effort       TEXT NOT NULL,
            review_mode            TEXT NOT NULL,
            network_enabled        INTEGER NOT NULL DEFAULT 0,
            api_key                TEXT NULL,
            is_default             INTEGER NOT NULL DEFAULT 0,
            created_at             TEXT NOT NULL,
            updated_at             TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ai_model_configs_default ON ai_model_configs(is_default, updated_at);

        CREATE TABLE IF NOT EXISTS ai_tasks (
            id                        TEXT PRIMARY KEY NOT NULL,
            profile_id                TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
            model_config_id           TEXT NULL REFERENCES ai_model_configs(id) ON DELETE SET NULL,
            prompt                    TEXT NOT NULL,
            execution_mode            TEXT NOT NULL,
            status                    TEXT NOT NULL,
            session_id                TEXT NULL,
            cdp_url                   TEXT NULL,
            requires_user_action      INTEGER NOT NULL DEFAULT 0,
            pending_approval_code     TEXT NULL,
            pending_approval_message  TEXT NULL,
            human_verification_policy TEXT NOT NULL,
            payment_policy            TEXT NOT NULL,
            created_at                TEXT NOT NULL,
            updated_at                TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ai_tasks_profile ON ai_tasks(profile_id);
        CREATE INDEX IF NOT EXISTS idx_ai_tasks_updated ON ai_tasks(updated_at);

        CREATE TABLE IF NOT EXISTS ai_task_events (
            id         TEXT PRIMARY KEY NOT NULL,
            task_id     TEXT NOT NULL REFERENCES ai_tasks(id) ON DELETE CASCADE,
            actor      TEXT NOT NULL DEFAULT 'system',
            kind       TEXT NOT NULL,
            action     TEXT NOT NULL DEFAULT 'event',
            message    TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ai_task_events_task ON ai_task_events(task_id, created_at);

        CREATE TABLE IF NOT EXISTS ai_profile_facts (
            id         TEXT PRIMARY KEY NOT NULL,
            profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
            key_name   TEXT NOT NULL,
            value_text TEXT NOT NULL,
            source     TEXT NOT NULL,
            sensitivity TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(profile_id, key_name)
        );
        CREATE INDEX IF NOT EXISTS idx_ai_profile_facts_profile ON ai_profile_facts(profile_id, updated_at);

        CREATE TABLE IF NOT EXISTS browser_extensions (
            id              TEXT PRIMARY KEY NOT NULL,
            name            TEXT NOT NULL,
            source          TEXT NOT NULL,
            store_url       TEXT NULL,
            store_id        TEXT NULL,
            version         TEXT NOT NULL DEFAULT '',
            install_path    TEXT NOT NULL,
            default_enabled INTEGER NOT NULL DEFAULT 0,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_browser_extensions_updated ON browser_extensions(updated_at);
        CREATE INDEX IF NOT EXISTS idx_browser_extensions_default ON browser_extensions(default_enabled);

        CREATE TABLE IF NOT EXISTS profile_extensions (
            profile_id   TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
            extension_id TEXT NOT NULL REFERENCES browser_extensions(id) ON DELETE CASCADE,
            created_at   TEXT NOT NULL,
            PRIMARY KEY(profile_id, extension_id)
        );
        CREATE INDEX IF NOT EXISTS idx_profile_extensions_extension ON profile_extensions(extension_id);

        CREATE TABLE IF NOT EXISTS local_api_settings (
            id          TEXT PRIMARY KEY NOT NULL,
            enabled     INTEGER NOT NULL DEFAULT 0,
            port        INTEGER NOT NULL DEFAULT 17688,
            bind_mode   TEXT NOT NULL DEFAULT 'loopback',
            updated_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS local_api_keys (
            id           TEXT PRIMARY KEY NOT NULL,
            name         TEXT NOT NULL,
            key_secret   TEXT NOT NULL,
            suffix       TEXT NOT NULL,
            created_at   TEXT NOT NULL,
            updated_at   TEXT NOT NULL,
            last_used_at TEXT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_local_api_keys_updated ON local_api_keys(updated_at);
        "#,
    )?;

    // Upgrade path for databases created before the fingerprint column existed. ADD COLUMN is
    // not idempotent, so a duplicate-column error here just means we're already current.
    if let Err(err) = conn.execute(
        "ALTER TABLE profiles ADD COLUMN fingerprint TEXT NOT NULL DEFAULT '{}'",
        [],
    ) {
        let msg = err.to_string();
        if !msg.contains("duplicate column name") {
            return Err(err);
        }
    }
    if let Err(err) = conn.execute(
        "ALTER TABLE profiles ADD COLUMN startup_page TEXT NOT NULL DEFAULT 'veil'",
        [],
    ) {
        let msg = err.to_string();
        if !msg.contains("duplicate column name") {
            return Err(err);
        }
    }
    if let Err(err) = conn.execute(
        "ALTER TABLE profiles ADD COLUMN startup_url TEXT NOT NULL DEFAULT ''",
        [],
    ) {
        let msg = err.to_string();
        if !msg.contains("duplicate column name") {
            return Err(err);
        }
    }
    if let Err(err) = conn.execute(
        "ALTER TABLE ai_tasks ADD COLUMN model_config_id TEXT NULL REFERENCES ai_model_configs(id) ON DELETE SET NULL",
        [],
    ) {
        let msg = err.to_string();
        if !msg.contains("duplicate column name") {
            return Err(err);
        }
    }
    if let Err(err) = conn.execute(
        "ALTER TABLE ai_task_events ADD COLUMN actor TEXT NOT NULL DEFAULT 'system'",
        [],
    ) {
        let msg = err.to_string();
        if !msg.contains("duplicate column name") {
            return Err(err);
        }
    }
    if let Err(err) = conn.execute(
        "ALTER TABLE ai_task_events ADD COLUMN action TEXT NOT NULL DEFAULT 'event'",
        [],
    ) {
        let msg = err.to_string();
        if !msg.contains("duplicate column name") {
            return Err(err);
        }
    }
    for sql in [
        "ALTER TABLE proxies ADD COLUMN detected_ip TEXT NULL",
        "ALTER TABLE proxies ADD COLUMN country_code TEXT NULL",
        "ALTER TABLE proxies ADD COLUMN region TEXT NULL",
        "ALTER TABLE proxies ADD COLUMN city TEXT NULL",
        "ALTER TABLE proxies ADD COLUMN timezone TEXT NULL",
    ] {
        if let Err(err) = conn.execute(sql, []) {
            let msg = err.to_string();
            if !msg.contains("duplicate column name") {
                return Err(err);
            }
        }
    }
    Ok(())
}
