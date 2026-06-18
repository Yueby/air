use url::Url;

const SENSITIVE_KEYS: &[&str] = &[
    "secret",
    "token",
    "password",
    "authorization",
    "authentication",
];

pub fn redact_log_value(input: &str) -> String {
    let mut output = input.to_owned();
    for key in SENSITIVE_KEYS {
        output = redact_key_value(&output, key);
    }
    redact_url_queries(&output)
}

fn redact_key_value(input: &str, key: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut result = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(relative) = lower[cursor..].find(key) {
        let key_start = cursor + relative;
        let key_end = key_start + key.len();
        result.push_str(&input[cursor..key_end]);
        let mut value_start = key_end;
        while let Some(ch) = input[value_start..].chars().next() {
            if ch == ':' || ch == '=' || ch.is_whitespace() || ch == '"' || ch == '\'' {
                result.push(ch);
                value_start += ch.len_utf8();
            } else {
                break;
            }
        }
        let mut value_end = value_start;
        while let Some(ch) = input[value_end..].chars().next() {
            if ch == '&' || ch == ',' || ch == '}' || ch == ']' || ch.is_whitespace() {
                break;
            }
            value_end += ch.len_utf8();
        }
        if value_end > value_start {
            result.push_str("***");
        }
        cursor = value_end;
    }
    result.push_str(&input[cursor..]);
    result
}

fn redact_url_queries(input: &str) -> String {
    input
        .split_whitespace()
        .map(|part| {
            let trimmed = part.trim_matches(|ch| ch == ',' || ch == ';');
            match Url::parse(trimmed) {
                Ok(mut url) if url.query().is_some() => {
                    let keys: Vec<String> =
                        url.query_pairs().map(|(key, _)| key.into_owned()).collect();
                    url.query_pairs_mut().clear();
                    for key in keys {
                        url.query_pairs_mut().append_pair(&key, "***");
                    }
                    part.replace(trimmed, url.as_str())
                }
                _ => part.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_named_secret_fields() {
        let redacted =
            redact_log_value("secret=abc token: def password=\"p1\" authorization Bearer");
        assert!(!redacted.contains("abc"));
        assert!(!redacted.contains("def"));
        assert!(!redacted.contains("p1"));
        assert!(redacted.contains("secret=***"));
    }

    #[test]
    fn redacts_mihomo_authentication_fields() {
        let redacted = redact_log_value(r#"{"authentication":["user:password"]}"#);

        assert!(redacted.contains("authentication"));
        assert!(!redacted.contains("user:password"));
    }

    #[test]
    fn redacts_subscription_url_query_values() {
        let redacted = redact_log_value("fetch https://example.test/sub?token=abc&user=bob");
        assert!(redacted.contains("token=***"));
        assert!(redacted.contains("user=***"));
        assert!(!redacted.contains("abc"));
        assert!(!redacted.contains("bob"));
    }
}
