//! DSN (Data Source Name) parser for CUBRID connection strings.
//!
//! # Format
//!
//! ```text
//! cubrid://[user[:password]]@host[:port]/database[?autocommit=true&timeout=30]
//! ```

use std::time::Duration;

/// Parsed DSN (Data Source Name) for a CUBRID connection.
#[derive(Debug, Clone)]
pub struct Dsn {
    /// Database host.
    pub host: String,
    /// Broker port (default: 33000).
    pub port: u16,
    /// Database name.
    pub database: String,
    /// Database user (default: "").
    pub user: String,
    /// Database password (default: "").
    pub password: String,
    /// Auto-commit mode (default: true).
    pub auto_commit: bool,
    /// Connection timeout (default: 30s).
    pub timeout: Duration,
}

impl Default for Dsn {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 33000,
            database: String::new(),
            user: String::new(),
            password: String::new(),
            auto_commit: true,
            timeout: Duration::from_secs(30),
        }
    }
}

impl Dsn {
    /// Parse a CUBRID DSN string.
    ///
    /// # Format
    ///
    /// ```text
    /// cubrid://[user[:password]]@host[:port]/database[?autocommit=true&timeout=30]
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use cubrid_client::Dsn;
    ///
    /// let dsn = Dsn::parse("cubrid://dba:@localhost:33000/demodb").unwrap();
    /// assert_eq!(dsn.host, "localhost");
    /// assert_eq!(dsn.port, 33000);
    /// assert_eq!(dsn.database, "demodb");
    /// assert_eq!(dsn.user, "dba");
    /// assert_eq!(dsn.password, "");
    /// assert!(dsn.auto_commit);
    /// ```
    pub fn parse(dsn: &str) -> Result<Self, String> {
        // Validate scheme
        let rest = dsn
            .strip_prefix("cubrid://")
            .ok_or_else(|| format!("DSN must start with 'cubrid://', got: {dsn}"))?;

        let mut result = Dsn::default();

        // Split at '?' to separate query params
        let (main_part, query_part) = match rest.find('?') {
            Some(pos) => (&rest[..pos], Some(&rest[pos + 1..])),
            None => (rest, None),
        };

        // Split user_info@host_and_db
        let (user_part, host_and_db) = match main_part.rfind('@') {
            Some(pos) => (Some(&main_part[..pos]), &main_part[pos + 1..]),
            None => (None, main_part),
        };

        // Parse user:password
        if let Some(user_info) = user_part {
            match user_info.find(':') {
                Some(pos) => {
                    result.user = percent_decode(&user_info[..pos]);
                    result.password = percent_decode(&user_info[pos + 1..]);
                }
                None => {
                    result.user = percent_decode(user_info);
                }
            }
        }

        // Parse host[:port]/database
        let (host_port, db) = match host_and_db.find('/') {
            Some(pos) => (&host_and_db[..pos], &host_and_db[pos + 1..]),
            None => return Err("database name is required in DSN".to_string()),
        };

        if db.is_empty() {
            return Err("database name is required in DSN".to_string());
        }
        result.database = db.to_string();

        // Parse host[:port]
        // Handle IPv6 addresses [::1]:port
        if host_port.starts_with('[') {
            // IPv6
            match host_port.find(']') {
                Some(end) => {
                    result.host = host_port[1..end].to_string();
                    if end + 1 < host_port.len() && host_port.as_bytes()[end + 1] == b':' {
                        result.port = host_port[end + 2..]
                            .parse()
                            .map_err(|e| format!("invalid port: {e}"))?;
                    }
                }
                None => return Err("invalid IPv6 address in DSN".to_string()),
            }
        } else {
            match host_port.rfind(':') {
                Some(pos) => {
                    let host = &host_port[..pos];
                    if !host.is_empty() {
                        result.host = host.to_string();
                    }
                    result.port = host_port[pos + 1..]
                        .parse()
                        .map_err(|e| format!("invalid port: {e}"))?;
                }
                None => {
                    if !host_port.is_empty() {
                        result.host = host_port.to_string();
                    }
                }
            }
        }

        // Parse query parameters
        if let Some(qs) = query_part {
            for param in qs.split('&') {
                if let Some(eq_pos) = param.find('=') {
                    let key = &param[..eq_pos];
                    let val = &param[eq_pos + 1..];
                    match key {
                        "autocommit" => {
                            result.auto_commit = matches!(val, "true" | "1" | "yes");
                        }
                        "timeout" => {
                            result.timeout = Duration::from_secs(
                                val.parse::<u64>()
                                    .map_err(|e| format!("invalid timeout: {e}"))?,
                            );
                        }
                        _ => {} // ignore unknown params
                    }
                }
            }
        }

        Ok(result)
    }
}

/// Simple percent-decode (handles %XX sequences).
fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                result.push(byte as char);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let dsn = Dsn::parse("cubrid://dba:@localhost:33000/demodb").unwrap();
        assert_eq!(dsn.host, "localhost");
        assert_eq!(dsn.port, 33000);
        assert_eq!(dsn.database, "demodb");
        assert_eq!(dsn.user, "dba");
        assert_eq!(dsn.password, "");
        assert!(dsn.auto_commit);
        assert_eq!(dsn.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_parse_with_password() {
        let dsn = Dsn::parse("cubrid://admin:secret@db.example.com:33000/production").unwrap();
        assert_eq!(dsn.host, "db.example.com");
        assert_eq!(dsn.user, "admin");
        assert_eq!(dsn.password, "secret");
        assert_eq!(dsn.database, "production");
    }

    #[test]
    fn test_parse_no_password() {
        let dsn = Dsn::parse("cubrid://dba@localhost/testdb").unwrap();
        assert_eq!(dsn.user, "dba");
        assert_eq!(dsn.password, "");
        assert_eq!(dsn.port, 33000); // default
        assert_eq!(dsn.database, "testdb");
    }

    #[test]
    fn test_parse_no_user() {
        let dsn = Dsn::parse("cubrid://localhost:33000/testdb").unwrap();
        assert_eq!(dsn.host, "localhost");
        assert_eq!(dsn.port, 33000);
        assert_eq!(dsn.user, "");
        assert_eq!(dsn.database, "testdb");
    }

    #[test]
    fn test_parse_autocommit_false() {
        let dsn = Dsn::parse("cubrid://dba:@localhost:33000/demodb?autocommit=false").unwrap();
        assert!(!dsn.auto_commit);
    }

    #[test]
    fn test_parse_timeout() {
        let dsn = Dsn::parse("cubrid://dba:@localhost:33000/demodb?timeout=60").unwrap();
        assert_eq!(dsn.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_parse_multiple_params() {
        let dsn =
            Dsn::parse("cubrid://dba:@localhost:33000/demodb?autocommit=false&timeout=120")
                .unwrap();
        assert!(!dsn.auto_commit);
        assert_eq!(dsn.timeout, Duration::from_secs(120));
    }

    #[test]
    fn test_parse_invalid_scheme() {
        let result = Dsn::parse("mysql://localhost/testdb");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cubrid://"));
    }

    #[test]
    fn test_parse_missing_database() {
        let result = Dsn::parse("cubrid://localhost:33000/");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("database"));
    }

    #[test]
    fn test_parse_no_slash() {
        let result = Dsn::parse("cubrid://localhost:33000");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_percent_encoded_password() {
        let dsn = Dsn::parse("cubrid://user:p%40ss@localhost/db").unwrap();
        assert_eq!(dsn.user, "user");
        assert_eq!(dsn.password, "p@ss");
    }

    #[test]
    fn test_parse_custom_port() {
        let dsn = Dsn::parse("cubrid://dba@localhost:44000/mydb").unwrap();
        assert_eq!(dsn.port, 44000);
    }

    #[test]
    fn test_parse_default_host() {
        let dsn = Dsn::parse("cubrid://dba@:33000/mydb").unwrap();
        assert_eq!(dsn.host, "localhost");
    }

    #[test]
    fn test_default_values() {
        let dsn = Dsn::default();
        assert_eq!(dsn.host, "localhost");
        assert_eq!(dsn.port, 33000);
        assert!(dsn.database.is_empty());
        assert!(dsn.user.is_empty());
        assert!(dsn.password.is_empty());
        assert!(dsn.auto_commit);
        assert_eq!(dsn.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_parse_autocommit_yes() {
        let dsn = Dsn::parse("cubrid://dba:@localhost:33000/db?autocommit=yes").unwrap();
        assert!(dsn.auto_commit);
    }

    #[test]
    fn test_parse_autocommit_1() {
        let dsn = Dsn::parse("cubrid://dba:@localhost:33000/db?autocommit=1").unwrap();
        assert!(dsn.auto_commit);
    }

    #[test]
    fn test_parse_unknown_params_ignored() {
        let dsn = Dsn::parse("cubrid://dba:@localhost:33000/db?unknown=value").unwrap();
        assert_eq!(dsn.database, "db");
    }

    #[test]
    fn test_clone() {
        let dsn = Dsn::parse("cubrid://dba:secret@localhost:33000/demodb").unwrap();
        let cloned = dsn.clone();
        assert_eq!(cloned.host, "localhost");
        assert_eq!(cloned.user, "dba");
        assert_eq!(cloned.password, "secret");
    }

    #[test]
    fn test_debug() {
        let dsn = Dsn::parse("cubrid://dba:@localhost:33000/demodb").unwrap();
        let debug = format!("{dsn:?}");
        assert!(debug.contains("localhost"));
        assert!(debug.contains("demodb"));
    }

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("hello%20world"), "hello world");
        assert_eq!(percent_decode("100%25"), "100%");
        assert_eq!(percent_decode("no_encoding"), "no_encoding");
        assert_eq!(percent_decode(""), "");
    }

    #[test]
    fn test_parse_ipv6_address() {
        let dsn = Dsn::parse("cubrid://dba:@[::1]:33000/testdb").unwrap();
        assert_eq!(dsn.host, "::1");
        assert_eq!(dsn.port, 33000);
        assert_eq!(dsn.database, "testdb");
    }

    #[test]
    fn test_parse_ipv6_default_port() {
        let dsn = Dsn::parse("cubrid://dba:@[::1]/testdb").unwrap();
        assert_eq!(dsn.host, "::1");
        assert_eq!(dsn.port, 33000);
    }

    #[test]
    fn test_parse_ipv6_invalid_no_closing_bracket() {
        let err = Dsn::parse("cubrid://dba:@[::1/testdb").unwrap_err();
        assert!(err.contains("invalid IPv6"));
    }

    #[test]
    fn test_parse_ipv6_invalid_port() {
        let err = Dsn::parse("cubrid://dba:@[::1]:notaport/testdb").unwrap_err();
        assert!(err.contains("invalid port"));
    }
}
