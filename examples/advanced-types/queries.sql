-- name: GetEvent :one
SELECT id, name, flags, event_window FROM events WHERE id = $1;

-- name: ListEvents :many
SELECT id, name, flags, event_window FROM events;
