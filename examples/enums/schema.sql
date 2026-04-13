CREATE TYPE status AS ENUM ('active', 'inactive', 'pending');

CREATE TABLE users (
    id     BIGSERIAL PRIMARY KEY,
    name   TEXT      NOT NULL,
    status status    NOT NULL DEFAULT 'active'
);
