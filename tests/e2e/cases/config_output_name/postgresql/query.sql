-- name: ListAuthors :many
SELECT id, name, bio
FROM authors
ORDER BY id;
