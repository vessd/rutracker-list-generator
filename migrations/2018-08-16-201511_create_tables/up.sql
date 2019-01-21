-- Your SQL goes here
CREATE TABLE topics (
  id INTEGER PRIMARY KEY NOT NULL,
  title VARCHAR(255) NOT NULL,
  author VARCHAR(25) NOT NULL
);

CREATE TABLE forums (
  id SMALLINT PRIMARY KEY NOT NULL,
  name VARCHAR(100) NOT NULL,
  tor_count INTEGER NOT NULL,
  tor_size_bytes DOUBLE NOT NULL,
  topic_id INTEGER NOT NULL,
  FOREIGN KEY(topic_id) REFERENCES topics(id)
);

CREATE TABLE torrents (
  topic_id INTEGER PRIMARY KEY NOT NULL,
  forum_id SMALLINT NOT NULL,
  poster_id INTEGER NOT NULL,
  title VARCHAR(255) NOT NULL,
  hash VARCHAR(64) UNIQUE NOT NULL,
  size DOUBLE NOT NULL,
  reg_time DATETIME NOT NULL,
  status SMALLINT NOT NULL,
  seeders SMALLINT NOT NULL,
  FOREIGN KEY(forum_id) REFERENCES forums(id)
);

CREATE TABLE keeper_torrents (
  keeper VARCHAR(25) NOT NULL,
  topic_id INTEGER NOT NULL,
  PRIMARY KEY(keeper, topic_id),
  FOREIGN KEY(topic_id) REFERENCES torrents(topic_id)
);

CREATE TABLE local_torrents (
  hash VARCHAR(64) NOT NULL,
  status SMALLINT NOT NULL,
  url VARCHAR(255) NOT NULL,
  PRIMARY KEY(hash, url),
  FOREIGN KEY(hash) REFERENCES torrents(hash)
);
