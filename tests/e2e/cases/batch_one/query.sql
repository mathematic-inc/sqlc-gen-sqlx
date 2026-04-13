-- name: BatchGetAuthor :batchone
SELECT id, name, bio
FROM authors
WHERE id = $1;
