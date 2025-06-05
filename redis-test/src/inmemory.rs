use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
    strings: HashMap<Vec<u8>, Vec<u8>>, // simple string key/value store
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

    fn req_packed_commands(&mut self, _cmd: &[u8], _offset: usize, _count: usize) -> RedisResult<Vec<Value>> {
        Err(RedisError::from((ErrorKind::ClientError, "pipelines are not yet supported by InMemoryRedisConnection")))
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