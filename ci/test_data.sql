INSERT INTO repos (host, owner, name) VALUES ('Github', 'RuairidhWilliamson', 'test');
INSERT INTO repos (host, owner, name) VALUES ('Github', 'RuairidhWilliamson', 'rain');

INSERT INTO runs (created_at, repo, commit, target) VALUES ('now', 1, 'abcdefg', 'ci');

INSERT INTO runs (created_at, repo, commit, target, dequeued_at, rain_version) VALUES ('now', 1, 'abcdefg', 'ci', 'now', '0.0.1');
INSERT INTO runs (created_at, repo, commit, target, dequeued_at, rain_version) VALUES ('now', 1, 'abcdefg', 'ci', 'now', '0.0.1');
INSERT INTO finished_runs (run, finished_at, status, execution_time_millis, output) VALUES (3, 'now', 'Success', 2000, 'unit');
