-- name: CreateAuthor :one
INSERT INTO authors (name, bio)
VALUES (sqlc.arg(name), sqlc.narg(bio))
RETURNING id, name, bio;
