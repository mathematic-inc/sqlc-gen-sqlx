-- name: BatchDeleteAuthor :batchexec
DELETE FROM authors
WHERE id = $1;
