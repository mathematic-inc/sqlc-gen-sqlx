-- name: GetOrderAddress :one
SELECT addr FROM orders WHERE id = $1;
