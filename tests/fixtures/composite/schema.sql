CREATE TYPE address AS (
    street TEXT,
    city   TEXT,
    zip    INT
);

CREATE TABLE orders (
    id      BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL,
    addr    address
);
