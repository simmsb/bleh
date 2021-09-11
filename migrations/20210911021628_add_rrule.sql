CREATE TABLE IF NOT EXISTS rrules
(
    id      INTEGER PRIMARY KEY NOT NULL,
    rule    TEXT                NOT NULL,
    message TEXT                NOT NULL,
    channel TEXT                NOT NULL,
    userid  TEXT                NOT NULL
);
