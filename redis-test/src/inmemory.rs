use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use redis::{ConnectionLike, ErrorKind, RedisError, RedisResult, Value};

/// A very small, in-process, single-threaded in-memory Redis implementation that aims to provide
/// higher–fidelity semantics than `MockRedisConnection`.
///
/// It supports a subset of the most common string-oriented commands that are frequently used in
/// unit tests.  Currently the following commands are recognised (case-insensitively):
///
/// * `PING` … returns `PONG`
/// * `SET key value` … returns `OK`
/// * `GET key` … returns the previously stored bulk string or `Nil`
/// * `DEL key [key …]` … returns the number of keys removed
/// * `EXISTS key [key …]` … returns the number of keys that exist
/// * `INCR key` / `DECR key` … parses the existing value as an integer (default 0) and
///   increments / decrements it.  Returns the new integer value.
///
/// The implementation purposefully keeps the feature-set small to stay lightweight, but adding new
/// commands is straightforward – look for the big `match`-statement inside
/// `handle_single_command`.
#[derive(Clone, Default)]
pub struct InMemoryRedisConnection {
    state: Arc<Mutex<InnerDb>>, // shared so that clones of the connection see the same state
}

#[derive(Default)]
struct InnerDb {
    strings: HashMap<Vec<u8>, Vec<u8>>,           // key -> bytes
    lists: HashMap<Vec<u8>, Vec<Vec<u8>>>,        // key -> Vec<elem>
    sets: HashMap<Vec<u8>, HashSet<Vec<u8>>>,     // key -> HashSet<elem>
    hashes: HashMap<Vec<u8>, HashMap<Vec<u8>, Vec<u8>>>, // key -> field map
    expirations: HashMap<Vec<u8>, Instant>,       // key -> deadline
}

impl InMemoryRedisConnection {
    pub fn new() -> Self {
        Self::default()
    }

    fn handle_single_command(&mut self, args: Vec<Vec<u8>>) -> RedisResult<Value> {
        if args.is_empty() {
            return Err(RedisError::from((ErrorKind::ClientError, "empty command")));
        }

        let cmd = String::from_utf8_lossy(&args[0]).to_ascii_uppercase();
        let mut db = self.state.lock().unwrap();
        // Remove expired keys first
        purge_expired(&mut db);
        match cmd.as_str() {
            "PING" => Ok(Value::SimpleString("PONG".into())),
            "SET" => {
                if args.len() < 3 {
                    Err(RedisError::from((ErrorKind::ClientError, "ERR wrong number of arguments for 'set' command")))
                } else {
                    db.strings.insert(args[1].clone(), args[2].clone());
                    Ok(Value::Okay)
                }
            }
            "GET" => {
                if args.len() != 2 {
                    Err(RedisError::from((ErrorKind::ClientError, "ERR wrong number of arguments for 'get' command")))
                } else {
                    match db.strings.get(&args[1]) {
                        Some(value) => Ok(Value::BulkString(value.clone())),
                        None => Ok(Value::Nil),
                    }
                }
            }
            "DEL" => {
                if args.len() < 2 {
                    Err(RedisError::from((ErrorKind::ClientError, "ERR wrong number of arguments for 'del' command")))
                } else {
                    let mut removed = 0i64;
                    for key in args.iter().skip(1) {
                        if db.strings.remove(key).is_some() {
                            removed += 1;
                        }
                    }
                    Ok(Value::Int(removed))
                }
            }
            "EXISTS" => {
                if args.len() < 2 {
                    Err(RedisError::from((ErrorKind::ClientError, "ERR wrong number of arguments for 'exists' command")))
                } else {
                    let mut exists = 0i64;
                    for key in args.iter().skip(1) {
                        if db.strings.contains_key(key) {
                            exists += 1;
                        }
                    }
                    Ok(Value::Int(exists))
                }
            }
            "INCR" | "DECR" => {
                let by: i64 = if cmd == "INCR" { 1 } else { -1 };
                if args.len() != 2 {
                    return Err(RedisError::from((ErrorKind::ClientError, "ERR wrong number of arguments for 'incr/decr' command")));
                }
                let key = &args[1];
                let current = db
                    .strings
                    .get(key)
                    .and_then(|v| String::from_utf8(v.clone()).ok())
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(0);
                let new_val = current + by;
                db.strings.insert(key.clone(), new_val.to_string().into_bytes());
                Ok(Value::Int(new_val))
            }
            "LPUSH" | "RPUSH" => {
                if args.len() < 3 {
                    return Err(wrong_arity("lpush/rpush"));
                }
                let key = &args[1];
                let mut list = db.lists.entry(key.clone()).or_default();
                let mut added = 0i64;
                for val in args.iter().skip(2) {
                    if cmd == "LPUSH" {
                        list.insert(0, val.clone());
                    } else {
                        list.push(val.clone());
                    }
                    added += 1;
                }
                Ok(Value::Int(list.len() as i64))
            }
            "LPOP" | "RPOP" => {
                if args.len() != 2 {
                    return Err(wrong_arity("lpop/rpop"));
                }
                let key = &args[1];
                let list_opt = db.lists.get_mut(key);
                if let Some(list) = list_opt {
                    let elem_opt = if cmd == "LPOP" {
                        if !list.is_empty() { Some(list.remove(0)) } else { None }
                    } else {
                        list.pop()
                    };
                    match elem_opt {
                        Some(e) => Ok(Value::BulkString(e)),
                        None => Ok(Value::Nil),
                    }
                } else {
                    Ok(Value::Nil)
                }
            }
            "LLEN" => {
                if args.len() != 2 {
                    return Err(wrong_arity("llen"));
                }
                let len = db.lists.get(&args[1]).map(|l| l.len()).unwrap_or(0);
                Ok(Value::Int(len as i64))
            }
            "LRANGE" => {
                if args.len() != 4 {
                    return Err(wrong_arity("lrange"));
                }
                let start = parse_i64(&args[2])?;
                let stop = parse_i64(&args[3])?;
                let list = db.lists.get(&args[1]);
                if list.is_none() {
                    return Ok(Value::Array(vec![]));
                }
                let list = list.unwrap();
                let len = list.len() as i64;
                // normalize indexes
                let start = if start < 0 { len + start } else { start };
                let stop = if stop < 0 { len + stop } else { stop };
                let start = start.max(0);
                let stop = stop.min(len - 1);
                if start > stop || start >= len {
                    return Ok(Value::Array(vec![]));
                }
                let slice: Vec<Value> = list[start as usize..=stop as usize]
                    .iter()
                    .cloned()
                    .map(Value::BulkString)
                    .collect();
                Ok(Value::Array(slice))
            }
            "SADD" => {
                if args.len() < 3 {
                    return Err(wrong_arity("sadd"));
                }
                let key = &args[1];
                let set = db.sets.entry(key.clone()).or_default();
                let mut added = 0i64;
                for member in args.iter().skip(2) {
                    if set.insert(member.clone()) {
                        added += 1;
                    }
                }
                Ok(Value::Int(added))
            }
            "SREM" => {
                if args.len() < 3 {
                    return Err(wrong_arity("srem"));
                }
                let key = &args[1];
                let set_opt = db.sets.get_mut(key);
                if set_opt.is_none() {
                    return Ok(Value::Int(0));
                }
                let set = set_opt.unwrap();
                let mut removed = 0i64;
                for member in args.iter().skip(2) {
                    if set.remove(member) {
                        removed += 1;
                    }
                }
                Ok(Value::Int(removed))
            }
            "SCARD" => {
                if args.len() != 2 {
                    return Err(wrong_arity("scard"));
                }
                let n = db.sets.get(&args[1]).map(|s| s.len()).unwrap_or(0);
                Ok(Value::Int(n as i64))
            }
            "SMEMBERS" => {
                if args.len() != 2 { return Err(wrong_arity("smembers")); }
                let members = db.sets.get(&args[1]);
                let array = match members {
                    Some(s) => s.iter().cloned().map(Value::BulkString).collect(),
                    None => vec![],
                };
                Ok(Value::Array(array))
            }
            "HSET" => {
                if args.len() < 4 || args.len() % 2 != 0 {
                    return Err(wrong_arity("hset"));
                }
                let key = &args[1];
                let hash = db.hashes.entry(key.clone()).or_default();
                let mut added = 0i64;
                let mut iter = args.iter().skip(2);
                while let Some(field) = iter.next() {
                    if let Some(value) = iter.next() {
                        if hash.insert(field.clone(), value.clone()).is_none() {
                            added += 1;
                        }
                    }
                }
                Ok(Value::Int(added))
            }
            "HGET" => {
                if args.len() != 3 { return Err(wrong_arity("hget")); }
                let key = &args[1];
                let field = &args[2];
                match db.hashes.get(key).and_then(|h| h.get(field)) {
                    Some(v) => Ok(Value::BulkString(v.clone())),
                    None => Ok(Value::Nil),
                }
            }
            "HDEL" => {
                if args.len() < 3 { return Err(wrong_arity("hdel")); }
                let key = &args[1];
                let hash_opt = db.hashes.get_mut(key);
                if hash_opt.is_none() { return Ok(Value::Int(0)); }
                let hash = hash_opt.unwrap();
                let mut removed = 0i64;
                for field in args.iter().skip(2) {
                    if hash.remove(field).is_some() { removed += 1; }
                }
                Ok(Value::Int(removed))
            }
            "HLEN" => {
                if args.len() != 2 { return Err(wrong_arity("hlen")); }
                let len = db.hashes.get(&args[1]).map(|h| h.len()).unwrap_or(0);
                Ok(Value::Int(len as i64))
            }
            "EXPIRE" => {
                if args.len() != 3 { return Err(wrong_arity("expire")); }
                let key = &args[1];
                if !key_exists(&db, key) { return Ok(Value::Int(0)); }
                let seconds = parse_i64(&args[2])?;
                db.expirations.insert(key.clone(), Instant::now() + Duration::from_secs(seconds as u64));
                Ok(Value::Int(1))
            }
            "TTL" => {
                if args.len() != 2 { return Err(wrong_arity("ttl")); }
                let key = &args[1];
                if !key_exists(&db, key) { return Ok(Value::Int(-2)); }
                if let Some(exp) = db.expirations.get(key) {
                    let now = Instant::now();
                    if *exp <= now { return Ok(Value::Int(-2)); }
                    let secs_left = exp.duration_since(now).as_secs() as i64;
                    Ok(Value::Int(secs_left))
                } else {
                    Ok(Value::Int(-1))
                }
            }
            _ => Err(RedisError::from((
                ErrorKind::ClientError,
                "TEST",
                format!("command '{cmd}' not implemented in InMemoryRedisConnection"),
            )))
        }
    }
}

impl ConnectionLike for InMemoryRedisConnection {
    fn req_packed_command(&mut self, cmd: &[u8]) -> RedisResult<Value> {
        // parse single command (RESP array)
        let value = redis::parse_redis_value(cmd)?;
        let args: Vec<Vec<u8>> = match value {
            Value::Array(items) => items
                .into_iter()
                .map(|v| match v {
                    Value::BulkString(bs) => Ok(bs),
                    Value::SimpleString(s) => Ok(s.into_bytes()),
                    Value::Int(i) => Ok(i.to_string().into_bytes()),
                    other => Err(RedisError::from((
                        ErrorKind::ClientError,
                        "TEST",
                        format!(
                            "unsupported redis type in command argument: {other:?}"
                        ),
                    ))),
                })
                .collect::<RedisResult<_>>()?,
            _ => {
                return Err(RedisError::from((ErrorKind::ClientError, "expected array in command")));
            }
        };
        self.handle_single_command(args)
    }

    fn req_packed_commands(&mut self, cmd: &[u8], _offset: usize, _count: usize) -> RedisResult<Vec<Value>> {
        // Parse sequential RESP values until we consume the whole slice.
        let mut pos = 0usize;
        let mut results = Vec::new();
        while pos < cmd.len() {
            // Use binary search to find minimal length that parses.
            let remaining = &cmd[pos..];
            let mut parser = redis::Parser::new();
            let mut consumed_opt = None;
            let mut hi = remaining.len();
            let mut lo = 1usize;
            while lo <= hi {
                let mid = (lo + hi) / 2;
                match parser.parse_value(&remaining[..mid]) {
                    Ok(_) => {
                        consumed_opt = Some(mid);
                        hi = mid - 1;
                    }
                    Err(_) => lo = mid + 1,
                }
            }
            let consumed = consumed_opt.ok_or_else(|| {
                RedisError::from((ErrorKind::ClientError, "TEST", "failed to parse pipeline".to_owned()))
            })?;

            let argv_val = parser.parse_value(&remaining[..consumed])?;
            let args_vec = match argv_val {
                Value::Array(items) => items
                    .into_iter()
                    .map(|v| match v {
                        Value::BulkString(bs) => Ok(bs),
                        Value::SimpleString(s) => Ok(s.into_bytes()),
                        Value::Int(i) => Ok(i.to_string().into_bytes()),
                        other => Err(RedisError::from((
                            ErrorKind::ClientError,
                            "TEST",
                            format!(
                                "unsupported redis type in command argument: {other:?}"
                            ),
                        ))),
                    })
                    .collect::<RedisResult<Vec<Vec<u8>>>>()?,
                _ => {
                    return Err(RedisError::from((ErrorKind::ClientError, "TEST", "expected array in pipeline command".to_owned())));
                }
            };
            let val = self.handle_single_command(args_vec)?;
            results.push(val);
            pos += consumed;
        }
        Ok(results)
    }

    fn get_db(&self) -> i64 {
        0
    }

    fn check_connection(&mut self) -> bool {
        true
    }

    fn is_open(&self) -> bool {
        true
    }
}

// --- helpers ---------------------------------------------------------------

fn wrong_arity(cmd: &str) -> RedisError {
    RedisError::from((
        ErrorKind::ClientError,
        "TEST",
        format!("ERR wrong number of arguments for '{cmd}' command"),
    ))
}

fn parse_i64(bytes: &[u8]) -> RedisResult<i64> {
    let s = String::from_utf8(bytes.to_vec()).map_err(|_| {
        RedisError::from((ErrorKind::ClientError, "TEST", "integer parse error".to_owned()))
    })?;
    s.parse::<i64>().map_err(|_| {
        RedisError::from((ErrorKind::ClientError, "TEST", "integer parse error".to_owned()))
    })
}

fn key_exists(db: &InnerDb, key: &[u8]) -> bool {
    db.strings.contains_key(key)
        || db.lists.contains_key(key)
        || db.sets.contains_key(key)
        || db.hashes.contains_key(key)
}

fn purge_expired(db: &mut InnerDb) {
    if db.expirations.is_empty() {
        return;
    }
    let now = Instant::now();
    let expired_keys: Vec<Vec<u8>> = db
        .expirations
        .iter()
        .filter_map(|(k, t)| if *t <= now { Some(k.clone()) } else { None })
        .collect();
    for k in expired_keys {
        db.expirations.remove(&k);
        db.strings.remove(&k);
        db.lists.remove(&k);
        db.sets.remove(&k);
        db.hashes.remove(&k);
    }
}