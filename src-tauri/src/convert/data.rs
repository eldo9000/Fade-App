use crate::{ConvertOptions, JobProgress};
use std::path::Path;
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

    let raw = std::fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let in_ext = Path::new(input_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let out_fmt = opts.output_format.to_lowercase();
    let pretty = opts.pretty_print.unwrap_or(true);
    let delimiter = opts.csv_delimiter.as_deref().unwrap_or(",");
    let delim_byte = delimiter.as_bytes().first().copied().unwrap_or(b',');

    let value: serde_json::Value = parse_input(&in_ext, &raw)?;
    let output = write_output(&out_fmt, &value, pretty, delim_byte)?;

    std::fs::write(output_path, output).map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_input(in_ext: &str, raw: &str) -> Result<serde_json::Value, String> {
    match in_ext {
        "json" | "ndjson" | "jsonl" => {
            serde_json::from_str(raw).map_err(|e| format!("JSON parse error: {e}"))
        }
        "yaml" | "yml" => serde_yaml::from_str(raw).map_err(|e| format!("YAML parse error: {e}")),
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

fn write_output(
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
        "yaml" => serde_yaml::to_string(value).map_err(|e| e.to_string()),
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
