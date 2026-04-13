-- name: BatchListAuthorsByBio :batchmany
SELECT id, name, bio
FROM authors
WHERE bio LIKE $1
ORDER BY id;
