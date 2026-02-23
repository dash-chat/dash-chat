CREATE KEYSPACE IF NOT EXISTS push_notifications
    WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};

CREATE TABLE IF NOT EXISTS push_notifications.fcm_tokens (
    public_key  text PRIMARY KEY,
    fcm_token   text,
    updated_at  timestamp
);
