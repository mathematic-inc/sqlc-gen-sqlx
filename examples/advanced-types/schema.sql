CREATE TABLE events (
    id     BIGSERIAL   PRIMARY KEY,
    name   TEXT        NOT NULL,
    flags  VARBIT(8)   NOT NULL,
    event_window TSTZRANGE   NOT NULL
);
