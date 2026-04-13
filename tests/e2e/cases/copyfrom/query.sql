-- name: CopyAuthors :copyfrom
INSERT INTO authors (name, bio)
VALUES ($1, $2);
