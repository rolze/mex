//! One-shot migration: add `partial_hash` column to the `media` table and
//! populate it for every existing row.
//!
//! Usage:
//!   migrate-partial-hashes [--db <path>] [--root <path>]
//!
//! Defaults:
//!   --db   .mex.db                           (current directory)
//!   --root target_root from ~/.config/mex/config.toml
//!
//! The tool is idempotent: rows that already have a `partial_hash` are
//! skipped, so it is safe to run more than once or to interrupt and re-run.

use anyhow::{Context, Result};
use mex::db::ensure_schema_v1;
use mex::import::{partial_hash_chunk, PARTIAL_HASH_BYTES};
use rusqlite::Connection;
use std::path::Path;

fn main() -> Result<()> {
    let (db_path, target_root) = parse_args()?;

    println!("DB:   {db_path}");
    println!("Root: {target_root}");
    println!();

    let conn = Connection::open(&db_path).with_context(|| format!("cannot open {db_path}"))?;

    // ── 1. Schema migration ───────────────────────────────────────────────────
    ensure_schema_v1(&conn)?;
    println!("Schema at version 1 (partial_hash column present).");

    // ── 2. Collect rows that need a partial_hash ──────────────────────────────
    let rows: Vec<(String, i64, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, file_size, target_path FROM media \
             WHERE status='moved' AND partial_hash IS NULL AND target_path IS NOT NULL",
        )?;
        let result: Vec<_> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    let total = rows.len();
    if total == 0 {
        println!("Nothing to migrate — all rows already have partial_hash.");
        return Ok(());
    }
    println!("Migrating {total} rows …");

    // ── 3. Compute and persist partial hashes ─────────────────────────────────
    // Commit in batches of 200 so a crash loses at most 200 rows of work.
    const BATCH: usize = 200;

    let mut done: usize = 0;
    let mut skipped: usize = 0;

    let mut batch_start = 0;
    while batch_start < total {
        let batch_end = (batch_start + BATCH).min(total);
        let batch = &rows[batch_start..batch_end];

        let tx = conn.unchecked_transaction()?;
        for (id, _size, rel_path) in batch {
            let abs_path = Path::new(&target_root).join(rel_path);
            match partial_hash_chunk(&abs_path, PARTIAL_HASH_BYTES) {
                Ok(phash) => {
                    tx.execute(
                        "UPDATE media SET partial_hash = ?1 WHERE id = ?2",
                        rusqlite::params![phash, id],
                    )?;
                    done += 1;
                }
                Err(e) => {
                    eprintln!("  skip {rel_path}: {e}");
                    skipped += 1;
                }
            }
        }
        tx.commit()?;

        print!("  {}/{total}\r", done + skipped);
        let _ = std::io::Write::flush(&mut std::io::stdout());

        batch_start = batch_end;
    }

    println!();
    println!("Done. {done} hashed, {skipped} skipped (file missing or unreadable).");
    Ok(())
}

fn parse_args() -> Result<(String, String)> {
    let mut db_path = ".mex.db".to_string();
    let mut target_root: Option<String> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => {
                db_path = args.next().context("--db requires a value")?;
            }
            "--root" => {
                target_root = Some(args.next().context("--root requires a value")?);
            }
            other => {
                anyhow::bail!("unknown argument: {other}\nUsage: migrate-partial-hashes [--db <path>] [--root <path>]");
            }
        }
    }

    let root = match target_root {
        Some(r) => r,
        None => {
            let cfg = mex::config::load_config();
            if cfg.target_root.is_empty() {
                anyhow::bail!(
                    "target_root not configured. Set it in ~/.config/mex/config.toml \
                     or pass --root <path>"
                );
            }
            cfg.target_root
        }
    };

    Ok((db_path, root))
}
