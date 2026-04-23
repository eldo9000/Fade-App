use crate::{tool_available, truncate_stderr, ConvertOptions, JobProgress};
use std::path::Path;
use std::process::{Command, Stdio};
use tauri::{Emitter, Window};

pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    opts: &ConvertOptions,
) -> Result<(), String> {
    let _ = window.emit(
        "job-progress",
        JobProgress {
            job_id: job_id.to_string(),
            percent: 0.0,
            message: "Converting data…".to_string(),
        },
    );

    let in_ext = Path::new(input_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let out_fmt = opts.output_format.to_lowercase();
    let pretty = opts.pretty_print.unwrap_or(true);
    let delimiter = opts.csv_delimiter.as_deref().unwrap_or(",");
    let delim_byte = delimiter.as_bytes().first().copied().unwrap_or(b',');

    // Binary data-nerd inputs don't parse as UTF-8 text — they have their
    // own dedicated dump paths and never go through the serde_json bridge.
    match in_ext.as_str() {
        "sqlite" | "sqlite3" | "db" => {
            return run_sqlite(input_path, output_path, &out_fmt, pretty, delim_byte);
        }
        "parquet" => {
            return run_parquet(input_path, output_path, &out_fmt);
        }
        _ => {}
    }

    let raw = std::fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let value: serde_json::Value = parse_input(&in_ext, &raw)?;
    let output = write_output(&out_fmt, &value, pretty, delim_byte)?;

    std::fs::write(output_path, output).map_err(|e| e.to_string())?;
    Ok(())
}

/// Dump a SQLite database to the target data format via the bundled rusqlite
/// crate (no external install). Default behaviour: dump the first user table.
/// Multi-table DBs get a warning on stderr so power users know to filter with
/// a CLI tool if they need a specific table. JSON output uses the shape
/// `{table_name: [...rows]}` so it's unambiguous even for single-table dumps.
fn run_sqlite(
    input_path: &str,
    output_path: &str,
    out_fmt: &str,
    pretty: bool,
    delim_byte: u8,
) -> Result<(), String> {
    let conn = rusqlite::Connection::open_with_flags(
        input_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|e| format!("SQLite open error: {e}"))?;

    // Enumerate user tables — exclude sqlite_* internal tables.
    let table_names: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master \
                 WHERE type='table' AND name NOT LIKE 'sqlite_%' \
                 ORDER BY name",
            )
            .map_err(|e| format!("SQLite schema query error: {e}"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("SQLite schema read error: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("SQLite schema row error: {e}"))?
    };

    if table_names.is_empty() {
        return Err("SQLite database contains no user tables".to_string());
    }
    if table_names.len() > 1 {
        eprintln!(
            "fade: SQLite DB has {} tables; dumping first ('{}'). \
             Others: {}",
            table_names.len(),
            table_names[0],
            table_names[1..].join(", ")
        );
    }
    let table = &table_names[0];

    let rows = dump_sqlite_table(&conn, table)?;
    let value = match out_fmt {
        // JSON wraps in {table: [...]} for clarity. CSV/TSV/XML flatten.
        "json" => {
            let mut obj = serde_json::Map::new();
            obj.insert(table.clone(), serde_json::Value::Array(rows));
            serde_json::Value::Object(obj)
        }
        _ => serde_json::Value::Array(rows),
    };

    let output = write_output(out_fmt, &value, pretty, delim_byte)?;
    std::fs::write(output_path, output).map_err(|e| e.to_string())?;
    Ok(())
}

fn dump_sqlite_table(
    conn: &rusqlite::Connection,
    table: &str,
) -> Result<Vec<serde_json::Value>, String> {
    // Quote the table name with double-quotes, doubling embedded quotes — safe
    // identifier quoting per SQLite syntax. User-supplied table names from the
    // sqlite_master enumeration should already be benign, but belt-and-braces.
    let quoted = format!("\"{}\"", table.replace('"', "\"\""));
    let sql = format!("SELECT * FROM {quoted}");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("SQLite prepare error: {e}"))?;
    let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();

    let mut rows_out = Vec::new();
    let mut rows = stmt
        .query([])
        .map_err(|e| format!("SQLite query error: {e}"))?;
    while let Some(row) = rows.next().map_err(|e| format!("SQLite row error: {e}"))? {
        let mut obj = serde_json::Map::new();
        for (i, name) in col_names.iter().enumerate() {
            let raw: rusqlite::types::Value = row
                .get(i)
                .map_err(|e| format!("SQLite cell read error: {e}"))?;
            obj.insert(name.clone(), sqlite_value_to_json(raw));
        }
        rows_out.push(serde_json::Value::Object(obj));
    }
    Ok(rows_out)
}

fn sqlite_value_to_json(v: rusqlite::types::Value) -> serde_json::Value {
    match v {
        rusqlite::types::Value::Null => serde_json::Value::Null,
        rusqlite::types::Value::Integer(i) => serde_json::Value::Number(i.into()),
        rusqlite::types::Value::Real(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        rusqlite::types::Value::Text(s) => serde_json::Value::String(s),
        // BLOB: represent as base64 to keep it JSON-safe.
        rusqlite::types::Value::Blob(b) => {
            use base64::Engine as _;
            serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(&b))
        }
    }
}

/// Parquet → CSV / JSON via the duckdb CLI. We prefer the CLI over a Rust
/// parquet crate to keep the binary lean — parquet pulls in arrow which is
/// tens of MB. duckdb's `COPY ... TO ... (FORMAT ...)` hits the disk directly.
///
/// JSON shape: duckdb's `FORMAT JSON` emits an array of row objects, matching
/// the sqlite JSON shape philosophically (rows are objects, wrapped).
fn run_parquet(input_path: &str, output_path: &str, out_fmt: &str) -> Result<(), String> {
    if !tool_available("duckdb") {
        let hint = if cfg!(target_os = "macos") {
            "brew install duckdb"
        } else {
            "apt install duckdb  (Debian/Ubuntu)\n  \
             or download from https://duckdb.org/docs/installation/"
        };
        return Err(format!(
            "duckdb not found in PATH.\n\nInstall with:\n  {hint}"
        ));
    }

    let (format_clause, _kind) = match out_fmt {
        "csv" => ("(FORMAT CSV, HEADER)", "csv"),
        "tsv" => ("(FORMAT CSV, HEADER, DELIMITER '\t')", "csv"),
        "json" => ("(FORMAT JSON, ARRAY true)", "json"),
        other => {
            return Err(format!(
                "Unsupported parquet output format: {other}. \
                 Allowed: csv, tsv, json"
            ));
        }
    };

    // duckdb escapes single quotes by doubling them in SQL string literals.
    let sql_in = input_path.replace('\'', "''");
    let sql_out = output_path.replace('\'', "''");
    let sql =
        format!("COPY (SELECT * FROM read_parquet('{sql_in}')) TO '{sql_out}' {format_clause};");

    let output = Command::new("duckdb")
        .arg(":memory:")
        .arg("-c")
        .arg(&sql)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run duckdb: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.trim().is_empty() {
            "duckdb parquet conversion failed".to_string()
        } else {
            truncate_stderr(&stderr)
        });
    }
    Ok(())
}

/// Public for integration tests (`src-tauri/tests/conversions.rs`).
pub fn parse_input(in_ext: &str, raw: &str) -> Result<serde_json::Value, String> {
    match in_ext {
        "json" | "ndjson" | "jsonl" => {
            serde_json::from_str(raw).map_err(|e| format!("JSON parse error: {e}"))
        }
        "yaml" | "yml" => {
            serde_yaml_ng::from_str(raw).map_err(|e| format!("YAML parse error: {e}"))
        }
        "toml" => {
            let v: toml::Value =
                toml::from_str(raw).map_err(|e| format!("TOML parse error: {e}"))?;
            serde_json::to_value(v).map_err(|e| e.to_string())
        }
        "csv" | "tsv" => parse_csv(raw, if in_ext == "tsv" { b'\t' } else { b',' }),
        "xml" => parse_xml(raw),
        _ => Err(format!("Unsupported input format: {in_ext}")),
    }
}

fn parse_csv(raw: &str, sep: u8) -> Result<serde_json::Value, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(sep)
        .from_reader(raw.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("CSV header error: {e}"))?
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut rows = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| format!("CSV row error: {e}"))?;
        let obj: serde_json::Map<String, serde_json::Value> = headers
            .iter()
            .zip(record.iter())
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.to_string())))
            .collect();
        rows.push(serde_json::Value::Object(obj));
    }
    Ok(serde_json::Value::Array(rows))
}

fn parse_xml(raw: &str) -> Result<serde_json::Value, String> {
    let mut reader = quick_xml::Reader::from_str(raw);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<(String, serde_json::Map<String, serde_json::Value>)> = Vec::new();
    let mut root_value: Option<serde_json::Value> = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                stack.push((name, serde_json::Map::new()));
            }
            Ok(quick_xml::events::Event::End(_)) => {
                if let Some((name, obj)) = stack.pop() {
                    let val = serde_json::Value::Object(obj);
                    if let Some((_, parent)) = stack.last_mut() {
                        parent.insert(name, val);
                    } else {
                        root_value = Some(val);
                    }
                }
            }
            Ok(quick_xml::events::Event::Text(e)) => {
                let text = e.unescape().map_err(|e| e.to_string())?.to_string();
                if !text.trim().is_empty() {
                    if let Some((_, obj)) = stack.last_mut() {
                        obj.insert("#text".to_string(), serde_json::Value::String(text));
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
        buf.clear();
    }
    Ok(root_value.unwrap_or(serde_json::Value::Object(serde_json::Map::new())))
}

/// Public for integration tests (`src-tauri/tests/conversions.rs`).
pub fn write_output(
    out_fmt: &str,
    value: &serde_json::Value,
    pretty: bool,
    delim_byte: u8,
) -> Result<String, String> {
    match out_fmt {
        "json" => {
            if pretty {
                serde_json::to_string_pretty(value).map_err(|e| e.to_string())
            } else {
                serde_json::to_string(value).map_err(|e| e.to_string())
            }
        }
        "yaml" => serde_yaml_ng::to_string(value).map_err(|e| e.to_string()),
        "toml" => write_toml(value),
        "csv" | "tsv" => write_csv(value, delim_byte),
        "xml" => Ok(write_xml(value, pretty)),
        _ => Err(format!("Unsupported output format: {out_fmt}")),
    }
}

fn write_toml(value: &serde_json::Value) -> Result<String, String> {
    // TOML requires a table at root; wrap arrays
    let toml_val: toml::Value = if value.is_array() {
        let mut map = toml::map::Map::new();
        let items: toml::Value = serde_json::from_value::<toml::Value>(
            serde_json::to_value(value).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        map.insert("items".to_string(), items);
        toml::Value::Table(map)
    } else {
        serde_json::from_value::<toml::Value>(
            serde_json::to_value(value).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?
    };
    toml::to_string_pretty(&toml_val).map_err(|e| e.to_string())
}

fn write_csv(value: &serde_json::Value, delim_byte: u8) -> Result<String, String> {
    let rows: Vec<serde_json::Value> = match value {
        serde_json::Value::Array(arr) => arr.clone(),
        other => vec![other.clone()],
    };
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim_byte)
        .from_writer(Vec::new());
    if let Some(serde_json::Value::Object(obj)) = rows.first() {
        let headers: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
        wtr.write_record(&headers).map_err(|e| e.to_string())?;
        for row in &rows {
            if let serde_json::Value::Object(obj) = row {
                let record: Vec<String> = headers
                    .iter()
                    .map(|h| {
                        obj.get(*h)
                            .map(|v| match v {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            })
                            .unwrap_or_default()
                    })
                    .collect();
                wtr.write_record(&record).map_err(|e| e.to_string())?;
            }
        }
    }
    String::from_utf8(wtr.into_inner().map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn write_xml(value: &serde_json::Value, pretty: bool) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    value_to_xml("root", value, &mut xml, "", pretty);
    xml
}

fn value_to_xml(key: &str, val: &serde_json::Value, out: &mut String, indent: &str, pretty: bool) {
    let nl = if pretty { "\n" } else { "" };
    let next_indent = if pretty {
        format!("{}  ", indent)
    } else {
        String::new()
    };
    match val {
        serde_json::Value::Object(obj) => {
            out.push_str(&format!("{}<{}>{}", indent, key, nl));
            for (k, v) in obj {
                if k == "#text" {
                    if let serde_json::Value::String(s) = v {
                        out.push_str(&format!("{}{}{}", next_indent, s, nl));
                    }
                } else {
                    value_to_xml(k, v, out, &next_indent, pretty);
                }
            }
            out.push_str(&format!("{}</{}>{}", indent, key, nl));
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                value_to_xml(key, item, out, indent, pretty);
            }
        }
        serde_json::Value::String(s) => {
            out.push_str(&format!("{}<{}>{}</{}>{}", indent, key, s, key, nl));
        }
        other => {
            out.push_str(&format!("{}<{}>{}</{}>{}", indent, key, other, key, nl));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_input, write_output};

    #[test]
    fn yaml_roundtrip_preserves_structure() {
        let src = "name: fade\ncount: 3\nitems:\n  - a\n  - b\n";
        let value = parse_input("yaml", src).expect("yaml parses");
        let out = write_output("yaml", &value, true, b',').expect("yaml writes");
        let reparsed = parse_input("yaml", &out).expect("yaml reparses");
        assert_eq!(value, reparsed);
        assert_eq!(reparsed["name"], "fade");
        assert_eq!(reparsed["count"], 3);
        assert_eq!(reparsed["items"][1], "b");
    }

    #[test]
    fn yml_extension_accepted() {
        let value = parse_input("yml", "k: v\n").expect("yml parses");
        assert_eq!(value["k"], "v");
    }
}
