-- name: GetUser :one
SELECT id, name, status FROM users WHERE id = $1;

-- name: ListUsersByStatus :many
SELECT id, name, status FROM users WHERE status = $1;

-- name: CreateUser :exec
INSERT INTO users (name, status) VALUES ($1, $2);
