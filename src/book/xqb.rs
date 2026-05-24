//! `xqb` 开局库：基于 fisherfan/xqbook 与 public-Xiangqi 的 XqbOpenBook 查询语义。

use super::{BookCandidate, BookResponse};
use parking_lot::{Mutex, RwLock};
use rusqlite::{Connection, OpenFlags, params};
use std::path::Path;
use std::sync::Arc;

type SharedConn = Arc<Mutex<Connection>>;

struct Cached {
    path: String,
    conn: SharedConn,
}

static CACHE: RwLock<Option<Cached>> = RwLock::new(None);

fn configure_readonly_pragmas(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        r#"
        PRAGMA query_only = ON;
        PRAGMA temp_store = MEMORY;
        PRAGMA mmap_size = 134217728;
        PRAGMA cache_size = -32768;
        "#,
    )
}

fn cached(path: &str) -> Result<SharedConn, String> {
    let path = path.trim();
    {
        let g = CACHE.read();
        if let Some(c) = g.as_ref()
            && c.path == path
        {
            return Ok(Arc::clone(&c.conn));
        }
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    configure_readonly_pragmas(&conn).map_err(|e| e.to_string())?;
    let arc = Arc::new(Mutex::new(conn));
    *CACHE.write() = Some(Cached {
        path: path.to_string(),
        conn: Arc::clone(&arc),
    });
    Ok(arc)
}

#[derive(Default)]
struct XqKey {
    key: Vec<u8>,
    mirror_ud: bool,
    mirror_lr: bool,
    rows: usize,
    cols: usize,
}

fn mirror_compare_value(cell: u8) -> i16 {
    if cell == u8::MAX { -1 } else { cell as i16 }
}

fn get_rows_and_cols(fen: &str) -> Option<(usize, usize)> {
    let board = fen.split(' ').next()?;
    let mut rows = 1usize;
    let mut cols = 0usize;
    let mut calc_cols = true;
    for ch in board.chars() {
        if ch == '/' {
            rows += 1;
            calc_cols = false;
        } else if calc_cols {
            if ch.is_ascii_digit() {
                cols += ch.to_digit(10)? as usize;
            } else {
                cols += 1;
            }
        }
    }
    Some((rows, cols))
}

fn map_piece(turn: char, ch: char) -> Option<u8> {
    let mapped = if turn == 'b' {
        if ch.is_ascii_alphabetic() {
            ((ch as u8) ^ 0x20) as char
        } else {
            ch
        }
    } else {
        ch
    };
    match mapped {
        'X' | 'x' => Some(0),
        'R' => Some(1),
        'N' => Some(2),
        'B' => Some(3),
        'A' => Some(4),
        'K' => Some(5),
        'C' => Some(6),
        'P' => Some(7),
        'r' => Some(9),
        'n' => Some(10),
        'b' => Some(11),
        'a' => Some(12),
        'k' => Some(13),
        'c' => Some(14),
        'p' => Some(15),
        _ => None,
    }
}

fn fen_to_key(fen: &str) -> Option<XqKey> {
    let board = fen.split(' ').next()?;
    let turn = fen.split(' ').nth(1)?.chars().next()?;
    let (rows, cols) = get_rows_and_cols(fen)?;
    let size = rows * cols;
    let mut ary = vec![u8::MAX; size];
    let mut index = 0usize;
    for ch in board.chars() {
        if ch == '/' {
            continue;
        }
        if ch.is_ascii_digit() {
            index += ch.to_digit(10)? as usize;
            continue;
        }
        let val = map_piece(turn, ch)?;
        if index >= size {
            return None;
        }
        ary[index] = val;
        index += 1;
    }

    let mut key = XqKey {
        key: Vec::new(),
        mirror_ud: false,
        mirror_lr: false,
        rows,
        cols,
    };

    if turn == 'b' {
        for row in 0..rows / 2 {
            for col in 0..cols {
                let i1 = row * cols + col;
                let i2 = (rows - 1 - row) * cols + (cols - 1 - col);
                ary.swap(i1, i2);
            }
        }
        key.mirror_ud = true;
    }

    let mut lr_done = false;
    for row in 0..rows {
        for col in 0..cols / 2 {
            let i1 = row * cols + col;
            let i2 = row * cols + (cols - 1 - col);
            if ary[i1] != ary[i2] {
                key.mirror_lr = mirror_compare_value(ary[i2]) > mirror_compare_value(ary[i1]);
                lr_done = true;
                break;
            }
        }
        if lr_done {
            break;
        }
    }
    if key.mirror_lr {
        for row in 0..rows {
            for col in 0..cols / 2 {
                let i1 = row * cols + col;
                let i2 = row * cols + (cols - 1 - col);
                ary.swap(i1, i2);
            }
        }
    }

    let mut buffer: u32 = 0;
    let buffer_bits = 32usize;
    let code_bits = 4usize;
    let mut bits = 0usize;
    for idx in 0..size {
        if ary[idx] == u8::MAX {
            bits += 1;
        } else {
            buffer |= 1 << (buffer_bits - bits - 1);
            buffer |= (ary[idx] as u32) << (buffer_bits - bits - 1 - code_bits);
            bits += 1 + code_bits;
        }
        let next_bits = if idx == size - 1 {
            0
        } else if ary[idx + 1] == u8::MAX {
            1
        } else {
            code_bits + 1
        };
        if idx == size - 1 || buffer_bits - bits < next_bits {
            while bits >= 8 {
                key.key.push(((buffer >> (buffer_bits - 8)) & 0xFF) as u8);
                buffer <<= 8;
                bits -= 8;
            }
            if idx == size - 1 && bits > 0 {
                key.key.push(((buffer >> (buffer_bits - 8)) & 0xFF) as u8);
                bits = 0;
            }
        }
    }
    Some(key)
}

fn mirror_move(mut mv: i32, mirror_ud: bool, mirror_lr: bool, rows: usize, cols: usize) -> i32 {
    if mirror_ud || mirror_lr {
        let mut from_row = (mv >> 12) as usize;
        let mut from_col = ((mv >> 8) & 0xF) as usize;
        let mut to_row = ((mv >> 4) & 0xF) as usize;
        let mut to_col = (mv & 0xF) as usize;
        if mirror_ud {
            from_row = rows - 1 - from_row;
            to_row = rows - 1 - to_row;
            from_col = cols - 1 - from_col;
            to_col = cols - 1 - to_col;
        }
        if mirror_lr {
            from_col = cols - 1 - from_col;
            to_col = cols - 1 - to_col;
        }
        mv = ((from_row as i32) << 12)
            | ((from_col as i32) << 8)
            | ((to_row as i32) << 4)
            | (to_col as i32);
    }
    mv
}

fn nibble_move_to_uci(mv: i32) -> Option<String> {
    let from_row = ((mv >> 12) & 0xF) as u8;
    let from_col = ((mv >> 8) & 0xF) as u8;
    let to_row = ((mv >> 4) & 0xF) as u8;
    let to_col = (mv & 0xF) as u8;
    fn sq(row: u8, col: u8) -> Option<String> {
        if row > 9 || col > 8 {
            return None;
        }
        let file = (b'a' + col) as char;
        let rank = (b'0' + (9 - row)) as char;
        Some(format!("{file}{rank}"))
    }
    Some(format!(
        "{}{}",
        sq(from_row, from_col)?,
        sq(to_row, to_col)?
    ))
}

fn rows_to_response(
    rows: Vec<(String, i32, i32, i32, i32)>,
    move_uci: Option<String>,
    source: &str,
) -> BookResponse {
    let candidates: Vec<BookCandidate> = rows
        .into_iter()
        .enumerate()
        .map(|(idx, (mv, score, win, draw, lost))| {
            let total = win + draw + lost;
            let winrate = if total > 0 {
                100.0 * (win as f64 + draw as f64 / 2.0) / total as f64
            } else {
                0.0
            };
            BookCandidate {
                move_uci: Some(mv),
                rank: Some((idx + 1) as i64),
                score: Some(score as f64),
                winrate: Some(winrate),
                winrate_raw: Some(format!("{winrate:.1}%")),
            }
        })
        .collect();
    BookResponse::with_move_eval(candidates, move_uci, source)
}

pub fn query_local(fen: &str, move_uci: Option<String>, path: &str) -> BookResponse {
    const SRC: &str = "xqb";
    let path = path.trim();
    if path.is_empty() || !Path::new(path).is_file() {
        return BookResponse::empty(SRC);
    }
    let Some(key) = fen_to_key(fen) else {
        let mut v = BookResponse::empty(SRC);
        v.error = Some("fen_parse_failed".to_string());
        return v;
    };
    let conn = match cached(path) {
        Ok(c) => c,
        Err(_) => {
            let mut v = BookResponse::empty(SRC);
            v.error = Some("open_xqb_failed".to_string());
            return v;
        }
    };
    let lock = conn.lock();
    let mut stmt = match lock
        .prepare_cached("SELECT Move, Score, Win, Draw, Lost, Valid FROM book WHERE key = ?1")
    {
        Ok(s) => s,
        Err(_) => {
            let mut v = BookResponse::empty(SRC);
            v.error = Some("prepare_failed".to_string());
            return v;
        }
    };
    let rows = match stmt.query_map(params![key.key], |r| {
        Ok((
            r.get::<_, i32>(0)?,
            r.get::<_, i32>(1)?,
            r.get::<_, i32>(2)?,
            r.get::<_, i32>(3)?,
            r.get::<_, i32>(4)?,
            r.get::<_, i32>(5)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => {
            let mut v = BookResponse::empty(SRC);
            v.error = Some("query_failed".to_string());
            return v;
        }
    };

    let mut parsed = Vec::new();
    for row in rows {
        let Ok((mv, score, win, draw, lost, valid)) = row else {
            continue;
        };
        if valid == 0 {
            continue;
        }
        let mirrored = mirror_move(mv, key.mirror_ud, key.mirror_lr, key.rows, key.cols);
        let Some(uci) = nibble_move_to_uci(mirrored) else {
            continue;
        };
        parsed.push((uci, score, win, draw, lost));
    }
    parsed.sort_by(|a, b| {
        let aw = a.2 + a.3 + a.4;
        let bw = b.2 + b.3 + b.4;
        let ar = if aw > 0 {
            (a.2 as f64 + a.3 as f64 / 2.0) / aw as f64
        } else {
            0.0
        };
        let br = if bw > 0 {
            (b.2 as f64 + b.3 as f64 / 2.0) / bw as f64
        } else {
            0.0
        };
        br.partial_cmp(&ar)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.cmp(&a.1))
    });
    rows_to_response(parsed, move_uci, SRC)
}

#[cfg(test)]
mod tests {
    use super::{fen_to_key, query_local};
    use rusqlite::{Connection, params};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("xq_xqb_{name}_{unique}.sqlite"))
    }

    #[test]
    fn xqb_query_returns_candidate_for_start_fen() {
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w";
        let key = fen_to_key(fen).expect("key");
        let path = temp_db_path("book");
        let conn = Connection::open(&path).expect("create xqb");
        conn.execute_batch(
            "CREATE TABLE book(
                Id INTEGER PRIMARY KEY AUTOINCREMENT,
                Key BLOB,
                Move INTEGER,
                Score INTEGER,
                Win INTEGER,
                Draw INTEGER,
                Lost INTEGER,
                Valid INTEGER,
                Memo TEXT
            );",
        )
        .expect("create book");
        conn.execute(
            "INSERT INTO book(Key, Move, Score, Win, Draw, Lost, Valid, Memo)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, 1, 'start')",
            params![key.key, 0x7174i32, 12i32, 6i32, 2i32, 2i32],
        )
        .expect("insert row");
        drop(conn);

        let res = query_local(fen, None, path.to_string_lossy().as_ref());
        assert_eq!(res.source, "xqb");
        assert_eq!(res.candidates.len(), 1);
        assert_eq!(res.candidates[0].move_uci.as_deref(), Some("b2e2"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn fen_to_key_handles_partial_final_byte_without_underflow() {
        let fen = "4k4/9/9/9/9/9/9/9/9/4K4 w";
        let key = fen_to_key(fen).expect("key");
        assert!(!key.key.is_empty());
    }

    #[test]
    fn xqb_query_supports_requested_move_eval() {
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w";
        let key = fen_to_key(fen).expect("key");
        let path = temp_db_path("move_eval");
        let conn = Connection::open(&path).expect("create xqb");
        conn.execute_batch(
            "CREATE TABLE book(
                Id INTEGER PRIMARY KEY AUTOINCREMENT,
                Key BLOB,
                Move INTEGER,
                Score INTEGER,
                Win INTEGER,
                Draw INTEGER,
                Lost INTEGER,
                Valid INTEGER,
                Memo TEXT
            );",
        )
        .expect("create book");
        conn.execute(
            "INSERT INTO book(Key, Move, Score, Win, Draw, Lost, Valid, Memo)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, 1, 'best')",
            params![key.key.clone(), 0x7174i32, 12i32, 8i32, 1i32, 1i32],
        )
        .expect("insert best row");
        conn.execute(
            "INSERT INTO book(Key, Move, Score, Win, Draw, Lost, Valid, Memo)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, 1, 'other')",
            params![key.key, 0x7173i32, 8i32, 4i32, 2i32, 2i32],
        )
        .expect("insert other row");
        drop(conn);

        let res = query_local(
            fen,
            Some("b2d2".to_string()),
            path.to_string_lossy().as_ref(),
        );
        assert_eq!(res.source, "xqb");
        assert_eq!(res.move_uci.as_deref(), Some("b2d2"));
        assert_eq!(res.best_move.as_deref(), Some("b2e2"));
        let eval = res
            .move_eval
            .expect("expected move eval for requested move");
        assert_eq!(eval.move_uci.as_deref(), Some("b2d2"));
        assert_eq!(eval.rank, Some(2));
        assert_eq!(eval.score, Some(8.0));
        assert_eq!(eval.winrate, Some(62.5));
        assert_eq!(eval.winrate_raw.as_deref(), Some("62.5%"));

        let _ = std::fs::remove_file(path);
    }
}
