//! 本地开局库：**SQLite**
//! 当前仅针对 **`bhobk`** 做正式支持与性能优化。
//! 其他历史 schema 先保留为“待实现/不支持”，避免继续分散优化精力。

use super::zobrist_openbook::{vmove_to_engine_uci, zobrist_pair_from_fen};
use super::{BookCandidate, BookResponse};
use parking_lot::{Mutex, RwLock};
use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OpenFlags};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BookKind {
    Bhobk,
    Unsupported,
}

struct Cached {
    path: String,
    kind: BookKind,
    conn: SharedConn,
    idx_conn: Option<SharedConn>,
}

static CACHE: RwLock<Option<Cached>> = RwLock::new(None);
type SharedConn = Arc<Mutex<Connection>>;

#[derive(Clone, Debug)]
pub struct ObkOptimizeResult {
    pub idx_path: String,
    pub indexed_rows: usize,
    pub skipped_rows: usize,
}

fn empty_response(source: &str) -> BookResponse {
    BookResponse::empty(source)
}

fn detect_kind(conn: &Connection) -> Option<BookKind> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .ok()?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .ok()?
        .filter_map(|x| x.ok())
        .collect();
    let set: std::collections::HashSet<_> = names.iter().map(|s| s.as_str()).collect();
    if set.contains("bhobk") {
        return Some(BookKind::Bhobk);
    }
    if set.contains("pfBook") || set.contains("xfen_book") {
        return Some(BookKind::Unsupported);
    }
    None
}

fn configure_readonly_pragmas(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        r#"
        PRAGMA query_only = ON;
        PRAGMA temp_store = MEMORY;
        PRAGMA mmap_size = 268435456;
        PRAGMA cache_size = -65536;
        "#,
    )
}

fn connection_for_path(path: &str) -> Result<(Connection, BookKind), String> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    configure_readonly_pragmas(&conn).map_err(|e| e.to_string())?;
    let kind = detect_kind(&conn).ok_or_else(|| "unsupported_sqlite_schema".to_string())?;
    Ok((conn, kind))
}

fn has_key_index_schema(conn: &Connection) -> bool {
    conn.prepare("SELECT 1 FROM key_index LIMIT 1").is_ok()
}

fn idx_path_for_source(path: &str) -> String {
    format!("{path}.idx")
}

/// 是否已有与 `.obk` 同名的 `.idx` 旁路索引。
pub fn has_idx_sidecar(obk_path: &str) -> bool {
    let path = obk_path.trim();
    if path.is_empty() {
        return false;
    }
    Path::new(&idx_path_for_source(path)).is_file()
}

fn invalidate_cache(path: &str) {
    let mut g = CACHE.write();
    if g.as_ref().is_some_and(|c| c.path == path) {
        *g = None;
    }
}

fn open_idx_sidecar(path: &str) -> Option<SharedConn> {
    let idx_path = idx_path_for_source(path);
    if !Path::new(&idx_path).is_file() {
        return None;
    }
    let conn = Connection::open_with_flags(
        &idx_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    configure_readonly_pragmas(&conn).ok()?;
    if !has_key_index_schema(&conn) {
        return None;
    }
    Some(Arc::new(Mutex::new(conn)))
}

type CachedLookup = (SharedConn, Option<SharedConn>, BookKind);

fn cached(path: &str) -> Result<CachedLookup, String> {
    let path = path.trim();
    {
        let g = CACHE.read();
        if let Some(c) = g.as_ref() {
            if c.path == path {
                return Ok((
                    Arc::clone(&c.conn),
                    c.idx_conn.as_ref().map(Arc::clone),
                    c.kind,
                ));
            }
        }
    }
    let (conn, kind) = connection_for_path(path)?;
    let arc = Arc::new(Mutex::new(conn));
    let idx_conn = if kind == BookKind::Bhobk {
        open_idx_sidecar(path)
    } else {
        None
    };
    *CACHE.write() = Some(Cached {
        path: path.to_string(),
        kind,
        conn: Arc::clone(&arc),
        idx_conn: idx_conn.as_ref().map(Arc::clone),
    });
    Ok((arc, idx_conn, kind))
}

#[derive(Debug)]
struct Row {
    move_uci: String,
    score: f64,
    winrate: f64,
    winrate_raw: String,
}

fn map_vscore_row(r: &rusqlite::Row) -> rusqlite::Result<(i32, i32, i32, i32, i32)> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
}

fn winrate_from_counts(w: i32, d: i32, l: i32) -> (f64, String) {
    let t = w + d + l;
    if t <= 0 {
        return (0.0, "0%".to_string());
    }
    let wr = 10000.0 * (w as f64 + d as f64 / 2.0) / t as f64;
    let p = (wr / 100.0).min(100.0);
    (p, format!("{:.1}%", p))
}

fn map_rows_from_stmt(
    rows: rusqlite::MappedRows<
        '_,
        impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<(i32, i32, i32, i32, i32)>,
    >,
    left_right_swap: bool,
) -> Result<Vec<Row>, rusqlite::Error> {
    let mut out = Vec::new();
    for row in rows {
        let (vscore, vwin, vdraw, vlost, vmove) = row?;
        let Some(mv) = vmove_to_engine_uci(vmove, left_right_swap) else {
            continue;
        };
        let mv: String = mv.chars().take(4).collect();
        if mv.len() < 4 {
            continue;
        }
        let (winrate, winrate_raw) = winrate_from_counts(vwin, vdraw, vlost);
        out.push(Row {
            move_uci: mv,
            score: vscore as f64,
            winrate,
            winrate_raw,
        });
    }
    Ok(out)
}

fn query_bhobk_rows(
    conn: &Connection,
    z: i64,
    left_right_swap: bool,
) -> Result<Vec<Row>, rusqlite::Error> {
    const BHOBK_SQL: &str =
        "SELECT vscore, vwin, vdraw, vlost, vmove FROM bhobk WHERE vkey = ? AND vvalid = 1";
    const BHOBK_SQL_REAL_FALLBACK: &str =
        "SELECT vscore, vwin, vdraw, vlost, vmove FROM bhobk WHERE cast(vkey as real) = ? AND vvalid = 1";

    let mut stmt = conn.prepare_cached(BHOBK_SQL)?;
    let rows = stmt.query_map(params![z], map_vscore_row)?;
    let out = map_rows_from_stmt(rows, left_right_swap)?;
    if !out.is_empty() || z >= 0 {
        return Ok(out);
    }

    // 某些历史 bhobk 负 key 使用 Java double 位型兼容；仅在整数索引命中失败时回退。
    let mut fallback_stmt = conn.prepare_cached(BHOBK_SQL_REAL_FALLBACK)?;
    let fallback_rows =
        fallback_stmt.query_map(params![f64::from_bits(z as u64)], map_vscore_row)?;
    map_rows_from_stmt(fallback_rows, left_right_swap)
}

fn query_idx_ids(idx_conn: &Connection, z: i64) -> Result<Vec<i64>, rusqlite::Error> {
    let mut stmt = idx_conn.prepare_cached("SELECT id FROM key_index WHERE vkey = ?")?;
    let rows = stmt.query_map(params![z], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn query_bhobk_rows_by_ids(
    conn: &Connection,
    ids: &[i64],
    left_right_swap: bool,
) -> Result<Vec<Row>, rusqlite::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for chunk in ids.chunks(900) {
        let placeholders = std::iter::repeat_n("?", chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT vscore, vwin, vdraw, vlost, vmove FROM bhobk WHERE id IN ({placeholders}) AND vvalid = 1"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(chunk.iter().copied()),
            map_vscore_row,
        )?;
        out.extend(map_rows_from_stmt(rows, left_right_swap)?);
    }
    Ok(out)
}

fn query_bhobk_rows_via_idx(
    conn: &Connection,
    idx_conn: &Connection,
    z: i64,
    left_right_swap: bool,
) -> Result<Vec<Row>, rusqlite::Error> {
    let ids = query_idx_ids(idx_conn, z)?;
    query_bhobk_rows_by_ids(conn, &ids, left_right_swap)
}

fn merge_zobrist_queries(
    conn: &Connection,
    idx_conn: Option<&Connection>,
    z1: i64,
    z2: i64,
) -> Result<Vec<Row>, rusqlite::Error> {
    let mut a = if let Some(idx_conn) = idx_conn {
        query_bhobk_rows_via_idx(conn, idx_conn, z1, false)?
    } else {
        query_bhobk_rows(conn, z1, false)?
    };
    let mut b = if let Some(idx_conn) = idx_conn {
        query_bhobk_rows_via_idx(conn, idx_conn, z2, true)?
    } else {
        query_bhobk_rows(conn, z2, true)?
    };
    a.append(&mut b);
    let mut best: HashMap<String, Row> = HashMap::new();
    for r in a {
        let k = r.move_uci.clone();
        if let Some(e) = best.get_mut(&k) {
            if r.winrate > e.winrate {
                *e = r;
            }
        } else {
            best.insert(k, r);
        }
    }
    Ok(best.into_values().collect())
}

fn rows_to_response(rows: Vec<Row>, move_uci: Option<String>, source: &str) -> BookResponse {
    let mut sorted = rows;
    sorted.sort_by(|a, b| {
        b.winrate
            .partial_cmp(&a.winrate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let candidates: Vec<BookCandidate> = sorted
        .iter()
        .take(5)
        .enumerate()
        .map(|(i, r)| BookCandidate {
            move_uci: Some(r.move_uci.clone()),
            rank: Some((i + 1) as i64),
            score: Some(r.score),
            winrate: Some(r.winrate),
            winrate_raw: Some(r.winrate_raw.clone()),
        })
        .collect();

    BookResponse::with_move_eval(candidates, move_uci, source)
}

fn normalize_vkey(v: ValueRef<'_>) -> Option<i64> {
    match v {
        ValueRef::Integer(n) => Some(n),
        ValueRef::Real(f) => {
            let bits = f.to_bits();
            Some(bits as i64)
        }
        _ => None,
    }
}

pub fn optimize_idx_sidecar(
    path: &str,
    mut on_progress: Option<&mut dyn FnMut(usize, usize)>,
) -> Result<ObkOptimizeResult, String> {
    let path = path.trim();
    if path.is_empty() || !Path::new(path).is_file() {
        return Err("book_not_found".to_string());
    }
    let (src_conn, kind) = connection_for_path(path)?;
    if kind != BookKind::Bhobk {
        return Err("unsupported_book_kind".to_string());
    }
    if has_idx_sidecar(path) {
        return Err("idx_already_exists".to_string());
    }

    let total_rows: usize = src_conn
        .query_row(
            "SELECT COUNT(*) FROM bhobk WHERE vkey IS NOT NULL",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n.max(0) as usize)
        .unwrap_or(0);
    if let Some(ref mut cb) = on_progress {
        cb(0, total_rows);
    }

    let idx_path = idx_path_for_source(path);
    let tmp_idx_path = format!("{idx_path}.tmp");
    let _ = std::fs::remove_file(&tmp_idx_path);

    let build = || -> Result<ObkOptimizeResult, String> {
        let progress_stride = total_rows.checked_div(200).unwrap_or(0).max(10_000).max(1);

        let (indexed_rows, skipped_rows) = {
            let dst = Connection::open(&tmp_idx_path).map_err(|e| e.to_string())?;
            dst.execute_batch(
                r#"
            PRAGMA journal_mode = OFF;
            PRAGMA synchronous = OFF;
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = -65536;
            CREATE TABLE key_index (id INTEGER, vkey INTEGER);
            "#,
            )
            .map_err(|e| e.to_string())?;

            let mut select = src_conn
                .prepare("SELECT id, vkey FROM bhobk WHERE vkey IS NOT NULL")
                .map_err(|e| e.to_string())?;
            let rows = select
                .query_map([], |r| {
                    let id = r.get::<_, i64>(0)?;
                    let vkey = normalize_vkey(r.get_ref(1)?);
                    Ok((id, vkey))
                })
                .map_err(|e| e.to_string())?;

            let tx = dst.unchecked_transaction().map_err(|e| e.to_string())?;
            let mut insert = tx
                .prepare("INSERT INTO key_index(id, vkey) VALUES(?1, ?2)")
                .map_err(|e| e.to_string())?;
            let mut indexed_rows = 0usize;
            let mut skipped_rows = 0usize;
            let mut processed = 0usize;
            for row in rows {
                let (id, vkey) = row.map_err(|e| e.to_string())?;
                processed += 1;
                if let Some(vkey) = vkey {
                    insert
                        .execute(params![id, vkey])
                        .map_err(|e| e.to_string())?;
                    indexed_rows += 1;
                } else {
                    skipped_rows += 1;
                }
                if let Some(ref mut cb) = on_progress {
                    if processed == total_rows
                        || (progress_stride > 0 && processed % progress_stride == 0)
                    {
                        cb(processed, total_rows);
                    }
                }
            }
            drop(insert);
            tx.execute("CREATE INDEX idx_vkey ON key_index(vkey)", [])
                .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            drop(select);
            drop(dst);
            (indexed_rows, skipped_rows)
        };

        if let Some(ref mut cb) = on_progress {
            cb(total_rows, total_rows.max(1));
        }

        if Path::new(&idx_path).is_file() {
            std::fs::remove_file(&idx_path).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&tmp_idx_path, &idx_path).map_err(|e| e.to_string())?;
        invalidate_cache(path);
        Ok(ObkOptimizeResult {
            idx_path,
            indexed_rows,
            skipped_rows,
        })
    };

    match build() {
        Ok(v) => Ok(v),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_idx_path);
            Err(e)
        }
    }
}

/// 查询本地 SQLite 库；未命中时 `candidates` 为空。
pub fn query_local(fen: &str, move_uci: Option<String>, path: &str) -> BookResponse {
    const SRC: &str = "obk";
    let path = path.trim();
    if path.is_empty() || !Path::new(path).is_file() {
        return empty_response(SRC);
    }

    let (conn, idx_conn, kind) = match cached(path) {
        Ok(x) => x,
        Err(_) => {
            let mut v = empty_response(SRC);
            v.error = Some("open_or_schema_failed".to_string());
            return v;
        }
    };

    let lock = conn.lock();
    let idx_lock = idx_conn.as_ref().map(|c| c.lock());

    if kind != BookKind::Bhobk {
        let mut v = empty_response(SRC);
        v.error = Some("unsupported_book_kind".to_string());
        return v;
    }

    let Some((z1, z2)) = zobrist_pair_from_fen(fen) else {
        let mut v = empty_response(SRC);
        v.error = Some("fen_parse_failed".to_string());
        return v;
    };

    let rows_res = merge_zobrist_queries(&lock, idx_lock.as_deref(), z1, z2);

    let rows = match rows_res {
        Ok(r) => r,
        Err(_) => {
            let mut v = empty_response(SRC);
            v.error = Some("query_failed".to_string());
            return v;
        }
    };

    rows_to_response(rows, move_uci, SRC)
}

#[cfg(test)]
mod tests {
    use super::{
        connection_for_path, has_idx_sidecar, optimize_idx_sidecar, query_bhobk_rows_via_idx,
        BookKind,
    };
    use rusqlite::Connection;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("xq_{name}_{unique}.sqlite"))
    }

    #[test]
    fn bhobk_schema_is_supported() {
        let path = temp_db_path("bhobk");
        let conn = Connection::open(&path).expect("create temp db");
        conn.execute_batch(
            "CREATE TABLE bhobk(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                vkey INTEGER,
                vmove INTEGER,
                vscore INTEGER,
                vwin INTEGER,
                vdraw INTEGER,
                vlost INTEGER,
                vvalid INTEGER,
                vmemo BLOB,
                vindex INTEGER
            );
            CREATE INDEX idxkey ON bhobk(vkey);",
        )
        .expect("create bhobk schema");
        drop(conn);

        let (_, kind) =
            connection_for_path(path.to_string_lossy().as_ref()).expect("open readonly");
        assert_eq!(kind, BookKind::Bhobk);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn legacy_schema_is_marked_unsupported() {
        let path = temp_db_path("xfen");
        let conn = Connection::open(&path).expect("create temp db");
        conn.execute_batch(
            "CREATE TABLE xfen_book(
                position_key TEXT,
                move_uci TEXT,
                weight REAL
            );",
        )
        .expect("create xfen schema");
        drop(conn);

        let (_, kind) =
            connection_for_path(path.to_string_lossy().as_ref()).expect("open readonly");
        assert_eq!(kind, BookKind::Unsupported);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn bhobk_idx_sidecar_hits_real_key_without_scan_path() {
        let src_path = temp_db_path("bhobk_src");
        let idx_path = PathBuf::from(format!("{}.idx", src_path.to_string_lossy()));

        let src = Connection::open(&src_path).expect("create src db");
        src.execute_batch(
            "CREATE TABLE bhobk(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                vkey INTEGER,
                vmove INTEGER,
                vscore INTEGER,
                vwin INTEGER,
                vdraw INTEGER,
                vlost INTEGER,
                vvalid INTEGER,
                vmemo BLOB,
                vindex INTEGER
            );
            CREATE INDEX idxkey ON bhobk(vkey);",
        )
        .expect("create bhobk schema");

        let z: i64 = -9223371910316222774;
        let z_real = f64::from_bits(z as u64);
        src.execute(
            "INSERT INTO bhobk(vkey, vmove, vscore, vwin, vdraw, vlost, vvalid, vmemo, vindex)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, 1, NULL, 0)",
            rusqlite::params![z_real, 0x6a67i32, 12i32, 8i32, 1i32, 1i32],
        )
        .expect("insert real-key row");
        let row_id = src.last_insert_rowid();
        drop(src);

        let idx = Connection::open(&idx_path).expect("create idx db");
        idx.execute_batch(
            "CREATE TABLE key_index (id INTEGER, vkey INTEGER);
             CREATE INDEX idx_vkey ON key_index(vkey);",
        )
        .expect("create idx schema");
        idx.execute(
            "INSERT INTO key_index(id, vkey) VALUES(?1, ?2)",
            rusqlite::params![row_id, z],
        )
        .expect("insert idx row");
        drop(idx);

        let src = Connection::open(&src_path).expect("reopen src");
        let idx = Connection::open(&idx_path).expect("reopen idx");
        let rows = query_bhobk_rows_via_idx(&src, &idx, z, false).expect("query via idx");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].score, 12.0);

        let _ = std::fs::remove_file(src_path);
        let _ = std::fs::remove_file(idx_path);
    }

    #[test]
    fn has_idx_sidecar_detects_existing_file() {
        let src_path = temp_db_path("has_idx");
        let idx_path = PathBuf::from(format!("{}.idx", src_path.to_string_lossy()));
        std::fs::write(&idx_path, b"x").expect("write idx");
        assert!(has_idx_sidecar(src_path.to_string_lossy().as_ref()));
        let _ = std::fs::remove_file(idx_path);
        assert!(!has_idx_sidecar(src_path.to_string_lossy().as_ref()));
    }

    #[test]
    fn optimize_idx_sidecar_rejects_when_idx_already_exists() {
        let src_path = temp_db_path("bhobk_has_idx");
        let idx_path = PathBuf::from(format!("{}.idx", src_path.to_string_lossy()));
        let conn = Connection::open(&src_path).expect("create src");
        conn.execute_batch(
            "CREATE TABLE bhobk(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                vkey INTEGER,
                vmove INTEGER,
                vscore INTEGER,
                vwin INTEGER,
                vdraw INTEGER,
                vlost INTEGER,
                vvalid INTEGER,
                vmemo BLOB,
                vindex INTEGER
            );",
        )
        .expect("schema");
        drop(conn);
        std::fs::write(&idx_path, b"x").expect("write idx");
        let err = optimize_idx_sidecar(src_path.to_string_lossy().as_ref(), None).unwrap_err();
        assert_eq!(err, "idx_already_exists");
        let _ = std::fs::remove_file(src_path);
        let _ = std::fs::remove_file(idx_path);
    }

    #[test]
    fn optimize_idx_sidecar_promotes_tmp_file_to_final_idx() {
        let src_path = temp_db_path("bhobk_optimize");
        let idx_path = PathBuf::from(format!("{}.idx", src_path.to_string_lossy()));
        let tmp_idx_path = PathBuf::from(format!("{}.tmp", idx_path.to_string_lossy()));

        let src = Connection::open(&src_path).expect("create src db");
        src.execute_batch(
            "CREATE TABLE bhobk(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                vkey INTEGER,
                vmove INTEGER,
                vscore INTEGER,
                vwin INTEGER,
                vdraw INTEGER,
                vlost INTEGER,
                vvalid INTEGER,
                vmemo BLOB,
                vindex INTEGER
            );
            CREATE INDEX idxkey ON bhobk(vkey);",
        )
        .expect("create bhobk schema");
        let z: i64 = -9223371910316222774;
        let z_real = f64::from_bits(z as u64);
        src.execute(
            "INSERT INTO bhobk(vkey, vmove, vscore, vwin, vdraw, vlost, vvalid, vmemo, vindex)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, 1, NULL, 0)",
            rusqlite::params![z_real, 0x1234i32, 12i32, 8i32, 1i32, 1i32],
        )
        .expect("insert real-key row");
        drop(src);

        let result =
            optimize_idx_sidecar(src_path.to_string_lossy().as_ref(), None).expect("optimize");
        assert_eq!(result.indexed_rows, 1);
        assert!(idx_path.is_file());
        assert!(!tmp_idx_path.exists());

        let idx = Connection::open(&idx_path).expect("open idx");
        let id_count: i64 = idx
            .query_row("SELECT COUNT(*) FROM key_index", [], |r| r.get(0))
            .expect("count rows");
        assert_eq!(id_count, 1);

        let _ = std::fs::remove_file(src_path);
        let _ = std::fs::remove_file(idx_path);
    }
}
