-- name: GetAuthor :one
SELECT id, name, bio
FROM authors
WHERE id = $1;
