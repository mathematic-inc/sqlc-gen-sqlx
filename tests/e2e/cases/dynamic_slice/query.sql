-- name: ListAuthorsByIds :many
SELECT id, name, bio
FROM authors
WHERE id IN (sqlc.slice('ids'))
ORDER BY id;
