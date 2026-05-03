CREATE TABLE IF NOT EXISTS view_events (
    username   TEXT NOT NULL,
    ip_hash    TEXT NOT NULL,
    last_seen  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (username, ip_hash)
);

CREATE INDEX IF NOT EXISTS idx_view_events_last_seen ON view_events(last_seen);
