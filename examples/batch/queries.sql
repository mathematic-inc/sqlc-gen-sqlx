-- name: BatchGetAuthor :batchone
SELECT id, name, bio FROM authors WHERE id = $1;

-- name: BatchDeleteAuthor :batchexec
DELETE FROM authors WHERE id = $1;
