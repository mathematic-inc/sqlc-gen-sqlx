-- name: DeleteAuthor :exec
DELETE FROM authors WHERE id = $1;

-- name: GetAuthor :one
SELECT id, name, bio FROM authors WHERE id = $1;
