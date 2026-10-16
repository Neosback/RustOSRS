use std::collections::BTreeMap;

/// Resolved gamevals with reserved base ranges and conflict-checked declarations.
#[derive(Debug, Clone, Default)]
pub struct GamevalSymbols {
    values: BTreeMap<String, i32>,
    base_max: BTreeMap<String, i32>,
    owners: BTreeMap<(String, i32), String>,
}
impl GamevalSymbols {
    pub fn resolve(&self, symbol: &str) -> Result<u32, String> {
        self.values
            .get(symbol)
            .and_then(|v| u32::try_from(*v).ok())
            .ok_or_else(|| format!("unresolved gameval {symbol}"))
    }
    /// Symbols of one table (`npc.hans` for table `npc`), ordered by ID then name.
    pub fn table_symbols(&self, table: &str) -> Vec<(u32, &str)> {
        let prefix = format!("{table}.");
        let mut rows: Vec<_> = self
            .values
            .iter()
            .filter(|(name, _)| name.starts_with(&prefix))
            .filter_map(|(name, value)| Some((u32::try_from(*value).ok()?, name.as_str())))
            .collect();
        rows.sort_unstable();
        rows
    }
    pub fn base_max(&self, table: &str) -> Option<i32> {
        self.base_max.get(table).copied()
    }

    /// Read Java DataInput's big-endian table/string format. Loading is atomic.
    /// Generated tables do not expand the reserved official ID ranges.
    pub fn load_dat(&mut self, bytes: &[u8], base: bool) -> Result<(), String> {
        let mut remaining = bytes;
        let mut parsed = self.clone();
        let tables = read_i32(&mut remaining)?;
        if !(0..=256).contains(&tables) {
            return Err("invalid gameval table count".into());
        }
        for _ in 0..tables {
            let table = read_string(&mut remaining)?.to_owned();
            let entries = read_i32(&mut remaining)?;
            if entries < 0 || entries as usize > remaining.len() / 3 {
                return Err("invalid gameval entry count".into());
            }
            for _ in 0..entries {
                let (key, value) = parse_line(read_string(&mut remaining)?)?;
                let symbol = format!("{table}.{key}");
                if let Some(old) = parsed.values.get(&symbol)
                    && *old != value
                {
                    return Err(format!("conflicting gameval {symbol}: {old} / {value}"));
                }
                parsed
                    .owners
                    .entry((table.clone(), value))
                    .or_insert_with(|| symbol.clone());
                parsed.values.insert(symbol, value);
                if base {
                    parsed
                        .base_max
                        .entry(table.clone())
                        .and_modify(|max| *max = (*max).max(value))
                        .or_insert(value);
                }
            }
        }
        if !remaining.is_empty() {
            return Err("trailing bytes in gamevals.dat".into());
        }
        *self = parsed;
        Ok(())
    }
    /// Apply explicit custom assignments, enforcing OpenRune's reserved-ID rule.
    pub fn declare(&mut self, table: &str, key: &str, value: i32) -> Result<(), String> {
        let symbol = format!(
            "{table}.{}",
            key.strip_prefix(&format!("{table}.")).unwrap_or(key)
        );
        let old = self.values.get(&symbol).copied();
        if value == -1 {
            self.values.entry(symbol).or_insert(-1);
            return Ok(());
        }
        if value < 0 || self.base_max.get(table).is_some_and(|max| value <= *max) {
            return Err(format!(
                "custom gameval {symbol}={value} overlaps reserved IDs"
            ));
        }
        if let Some(old) = old.filter(|v| *v >= 0) {
            return if old == value {
                Ok(())
            } else {
                Err(format!("conflicting gameval {symbol}: {old} / {value}"))
            };
        }
        if self.owners.contains_key(&(table.to_owned(), value)) {
            return Err(format!("duplicate custom gameval ID {table}:{value}"));
        }
        self.owners
            .insert((table.to_owned(), value), symbol.clone());
        self.values.insert(symbol, value);
        Ok(())
    }
    pub fn load_rscm(&mut self, table: &str, text: &str) -> Result<(), String> {
        let mut parsed = self.clone();
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let (key, value) = parse_line(line).map_err(|e| format!("line {}: {e}", index + 1))?;
            parsed.declare(table, &key, value)?;
        }
        *self = parsed;
        Ok(())
    }
    pub fn load_toml(&mut self, text: &str) -> Result<(), String> {
        let doc: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut parsed = self.clone();
        if let Some(tables) = doc.get("gamevals").and_then(toml::Value::as_table) {
            for (table, entries) in tables {
                let entries = entries.as_table().ok_or("gameval table expected")?;
                for (key, value) in entries {
                    let value = value
                        .as_integer()
                        .and_then(|v| i32::try_from(v).ok())
                        .ok_or("gameval must be i32")?;
                    parsed.declare(table, key, value)?;
                }
            }
        }
        *self = parsed;
        Ok(())
    }
}
fn read_i32(bytes: &mut &[u8]) -> Result<i32, String> {
    let value: [u8; 4] = bytes
        .get(..4)
        .ok_or("truncated gameval integer")?
        .try_into()
        .map_err(|_| "invalid integer")?;
    *bytes = &bytes[4..];
    Ok(i32::from_be_bytes(value))
}
fn read_string<'a>(bytes: &mut &'a [u8]) -> Result<&'a str, String> {
    let prefix: [u8; 2] = bytes
        .get(..2)
        .ok_or("truncated gameval string")?
        .try_into()
        .map_err(|_| "invalid length")?;
    let length = usize::try_from(i16::from_be_bytes(prefix))
        .map_err(|_| "negative gameval string length")?;
    let text = std::str::from_utf8(bytes.get(2..2 + length).ok_or("truncated gameval string")?)
        .map_err(|e| e.to_string())?;
    *bytes = &bytes[2 + length..];
    Ok(text)
}
fn parse_line(line: &str) -> Result<(String, i32), String> {
    let (key, value) = line
        .trim()
        .split_once('=')
        .ok_or_else(|| format!("invalid gameval entry {line:?}"))?;
    let value = value
        .trim()
        .parse()
        .map_err(|_| format!("invalid gameval value {line:?}"))?;
    if key.trim().is_empty() {
        return Err("empty gameval key".into());
    }
    Ok((key.trim().to_owned(), value))
}
