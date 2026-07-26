use std::{
    collections::HashMap,
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrecencyRank {
    pub score: u64,
    pub last_access: u64,
}

#[derive(Debug, Clone, Copy)]
struct Record {
    last_access: u64,
    access_count: u32,
}

impl Record {
    fn rank(self, now: u64) -> FrecencyRank {
        let age_hours = now.saturating_sub(self.last_access) / 3_600;
        let recency_weight = match age_hours {
            0..=4 => 100,
            5..=24 => 70,
            25..=168 => 50,
            169..=720 => 30,
            _ => 10,
        };
        FrecencyRank {
            score: recency_weight * u64::from(self.access_count.min(20)),
            last_access: self.last_access,
        }
    }
}

pub struct Frecency {
    connection: Connection,
    max_entries: usize,
}

impl Frecency {
    pub fn load(data_root: &Path, max_entries: usize) -> Result<Self> {
        fs::create_dir_all(data_root)
            .with_context(|| format!("failed to create {}", data_root.display()))?;
        let path = data_root.join("frecency.sqlite3");
        let connection = Connection::open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?;
        connection
            .busy_timeout(std::time::Duration::from_secs(2))
            .context("failed to configure frecency busy timeout")?;
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE IF NOT EXISTS frecency (
                    palette TEXT NOT NULL,
                    value TEXT NOT NULL,
                    last_access INTEGER NOT NULL,
                    access_count INTEGER NOT NULL,
                    PRIMARY KEY (palette, value)
                 );",
            )
            .with_context(|| format!("failed to initialize {}", path.display()))?;
        Ok(Self {
            connection,
            max_entries,
        })
    }

    pub fn scores(&self, palette: &str) -> Result<HashMap<String, FrecencyRank>> {
        self.scores_at(palette, current_timestamp())
    }

    pub fn record(&mut self, palette: &str, value: &str) -> Result<()> {
        self.record_at(palette, value, current_timestamp())
    }

    fn scores_at(&self, palette: &str, now: u64) -> Result<HashMap<String, FrecencyRank>> {
        let mut statement = self.connection.prepare(
            "SELECT value, last_access, access_count
                 FROM frecency WHERE palette = ?1",
        )?;
        let records = statement.query_map([palette], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        records
            .map(|record| {
                let (value, last_access, access_count) = record?;
                let record = Record {
                    last_access: last_access
                        .try_into()
                        .context("negative frecency timestamp")?,
                    access_count: access_count.try_into().context("invalid frecency count")?,
                };
                Ok((value, record.rank(now)))
            })
            .collect::<Result<_>>()
            .context("failed to read frecency scores")
    }

    fn record_at(&mut self, palette: &str, value: &str, now: u64) -> Result<()> {
        let now: i64 = now.try_into().context("frecency timestamp is too large")?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO frecency (palette, value, last_access, access_count)
             VALUES (?1, ?2, ?3, 1)
             ON CONFLICT (palette, value) DO UPDATE SET
               last_access = excluded.last_access,
               access_count = MIN(frecency.access_count + 1, 4294967295)",
            params![palette, value, now],
        )?;
        let count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM frecency WHERE palette = ?1",
            [palette],
            |row| row.get(0),
        )?;
        let max_entries: i64 = self
            .max_entries
            .try_into()
            .context("frecency entry limit is too large")?;
        if count > max_entries {
            let excess = count - max_entries;
            transaction.execute(
                "DELETE FROM frecency WHERE rowid IN (
                   SELECT rowid FROM frecency
                   WHERE palette = ?1
                   ORDER BY
                     (CASE
                       WHEN (?2 - last_access) / 3600 <= 4 THEN 100
                       WHEN (?2 - last_access) / 3600 <= 24 THEN 70
                       WHEN (?2 - last_access) / 3600 <= 168 THEN 50
                       WHEN (?2 - last_access) / 3600 <= 720 THEN 30
                       ELSE 10
                     END) * MIN(access_count, 20) ASC,
                     last_access ASC
                   LIMIT ?3
                 )",
                params![palette, now, excess],
            )?;
        }
        transaction.commit().context("failed to record frecency")
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frc_001_recent_frequent_entries_receive_higher_scores() {
        let now = 4_000_000;
        let recent = Record {
            last_access: now,
            access_count: 5,
        };
        let old = Record {
            last_access: now - 31 * 24 * 3_600,
            access_count: 20,
        };

        assert_eq!(recent.rank(now).score, 500);
        assert_eq!(old.rank(now).score, 200);
        assert!(recent.rank(now).score > old.rank(now).score);
    }

    #[test]
    fn frc_002_frecency_data_round_trips_by_palette() {
        let root = temp_root("roundtrip");
        let mut frecency = Frecency::load(&root, 100).unwrap();
        frecency.record_at("agents", "agent-1", 100).unwrap();
        frecency.record_at("files", "src/main.rs", 200).unwrap();
        drop(frecency);

        let loaded = Frecency::load(&root, 100).unwrap();

        assert!(
            loaded
                .scores_at("agents", 200)
                .unwrap()
                .contains_key("agent-1")
        );
        assert!(
            !loaded
                .scores_at("agents", 200)
                .unwrap()
                .contains_key("src/main.rs")
        );
        drop(loaded);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn frc_003_lowest_ranked_records_are_pruned() {
        let root = temp_root("prune");
        let mut frecency = Frecency::load(&root, 2).unwrap();
        frecency.record_at("agents", "old", 100).unwrap();
        frecency.record_at("agents", "middle", 200).unwrap();
        frecency.record_at("agents", "recent", 300).unwrap();

        let scores = frecency.scores_at("agents", 300).unwrap();

        assert_eq!(scores.len(), 2);
        assert!(!scores.contains_key("old"));
        drop(frecency);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn frc_008_concurrent_sessions_preserve_both_updates() {
        let root = temp_root("concurrent");
        let mut first = Frecency::load(&root, 100).unwrap();
        let mut second = Frecency::load(&root, 100).unwrap();

        first.record_at("agents", "one", 100).unwrap();
        second.record_at("agents", "two", 200).unwrap();

        let scores = first.scores_at("agents", 200).unwrap();
        assert!(scores.contains_key("one"));
        assert!(scores.contains_key("two"));
        drop((first, second));
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("vellum-frecency-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }
}
