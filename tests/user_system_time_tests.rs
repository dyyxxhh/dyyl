use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

fn eval_one(source: &str) -> Value {
    run_script(source, false)
        .values
        .into_iter()
        .next()
        .unwrap_or(Value::Empty)
}

#[test]
fn user_id_returns_nonempty_string() {
    let v = eval_one("user.id");
    match &v {
        Value::Str(s) => assert!(!s.is_empty(), "user.id should be nonempty"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn user_name_returns_nonempty_string() {
    let v = eval_one("user.name");
    match &v {
        Value::Str(s) => assert!(!s.is_empty(), "user.name should be nonempty"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn user_bash_success_returns_stdout() {
    let v = eval_one("user.bash printf dyyl-ok");
    assert_eq!(v, Value::Str("dyyl-ok".into()));
}

#[test]
fn user_bash_failure_returns_negative_one() {
    let v = eval_one("user.bash false");
    assert_eq!(v, Value::Num(-1));
}

#[test]
fn user_bash_exit_1_returns_negative_one() {
    let v = eval_one("user.bash exit 1");
    assert_eq!(v, Value::Num(-1));
}

#[test]
fn user_bash_captures_stdout() {
    let v = eval_one("user.bash printf hello-world");
    assert_eq!(v, Value::Str("hello-world".into()));
}

#[test]
fn system_os_returns_nonempty_string() {
    let v = eval_one("system.os");
    match &v {
        Value::Str(s) => assert!(!s.is_empty(), "system.os should be nonempty"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn system_arch_returns_nonempty_string() {
    let v = eval_one("system.arch");
    match &v {
        Value::Str(s) => assert!(!s.is_empty(), "system.arch should be nonempty"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn time_get_returns_reasonable_timestamp() {
    let v = eval_one("time.get");
    match &v {
        Value::Num(n) => assert!(*n > 1_000_000_000, "time.get should be > 1B"),
        other => panic!("expected Num, got {other:?}"),
    }
}

#[test]
fn time_now_matches_format() {
    let v = eval_one("time.now");
    match &v {
        Value::Str(s) => {
            assert_eq!(s.len(), 19, "time.now should be 19 chars: {s}");
            assert_eq!(&s[4..5], "-", "dash at position 4");
            assert_eq!(&s[7..8], "-", "dash at position 7");
            assert_eq!(&s[10..11], " ", "space at position 10");
            assert_eq!(&s[13..14], ":", "colon at position 13");
            assert_eq!(&s[16..17], ":", "colon at position 16");
        }
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn time_year_in_range() {
    let v = eval_one("time.year");
    match &v {
        Value::Num(n) => {
            assert!(*n >= 2024, "year >= 2024");
            assert!(*n <= 2100, "year <= 2100");
        }
        other => panic!("expected Num, got {other:?}"),
    }
}

#[test]
fn time_month_in_range() {
    let v = eval_one("time.month");
    match &v {
        Value::Num(n) => {
            assert!(*n >= 1, "month >= 1");
            assert!(*n <= 12, "month <= 12");
        }
        other => panic!("expected Num, got {other:?}"),
    }
}

#[test]
fn time_day_in_range() {
    let v = eval_one("time.day");
    match &v {
        Value::Num(n) => {
            assert!(*n >= 1, "day >= 1");
            assert!(*n <= 31, "day <= 31");
        }
        other => panic!("expected Num, got {other:?}"),
    }
}

#[test]
fn time_hour_in_range() {
    let v = eval_one("time.hour");
    match &v {
        Value::Num(n) => {
            assert!(*n >= 0, "hour >= 0");
            assert!(*n <= 23, "hour <= 23");
        }
        other => panic!("expected Num, got {other:?}"),
    }
}

#[test]
fn time_minute_in_range() {
    let v = eval_one("time.minute");
    match &v {
        Value::Num(n) => {
            assert!(*n >= 0, "minute >= 0");
            assert!(*n <= 59, "minute <= 59");
        }
        other => panic!("expected Num, got {other:?}"),
    }
}

#[test]
fn time_second_in_range() {
    let v = eval_one("time.second");
    match &v {
        Value::Num(n) => {
            assert!(*n >= 0, "second >= 0");
            assert!(*n <= 59, "second <= 59");
        }
        other => panic!("expected Num, got {other:?}"),
    }
}

#[test]
fn time_weekday_in_range() {
    let v = eval_one("time.weekday");
    match &v {
        Value::Num(n) => {
            assert!(*n >= 1, "weekday >= 1 (Monday)");
            assert!(*n <= 7, "weekday <= 7 (Sunday)");
        }
        other => panic!("expected Num, got {other:?}"),
    }
}

#[test]
fn time_weekday_name_nonempty() {
    let v = eval_one("time.weekday.name");
    match &v {
        Value::Str(s) => assert!(!s.is_empty(), "weekday name nonempty"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn time_format_custom_string() {
    let v = eval_one("time.format YYYY");
    match &v {
        Value::Str(s) => assert_eq!(s.len(), 4, "YYYY format should be 4 chars: {s}"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn time_diff_known_timestamps() {
    let v = eval_one("time.diff 0, 86400");
    assert_eq!(v, Value::Num(86400), "diff of 86400 seconds");
}

#[test]
fn time_add_known_timestamp() {
    let v = eval_one("time.add 0, 86400");
    assert_eq!(v, Value::Num(86400), "add 86400 to 0");
}

#[test]
fn user_bash_unknown_subcommand_is_error() {
    let v = eval_one("user.config");
    assert_eq!(v, Value::Num(-1), "unknown user command -> sentinel");
}

#[test]
fn system_unknown_subcommand_is_error() {
    let v = eval_one("system.unknown");
    assert_eq!(v, Value::Num(-1), "unknown system command -> sentinel");
}

#[test]
fn time_unknown_subcommand_is_error() {
    let v = eval_one("time.unknown");
    assert_eq!(v, Value::Num(-1), "unknown time command -> sentinel");
}

#[test]
fn time_wait_returns_value() {
    let v = eval_one("time.wait 10");
    assert_eq!(v, Value::Num(10), "time.wait returns the ms value");
}

#[test]
fn time_wait_zero_ms() {
    let v = eval_one("time.wait 0");
    assert_eq!(v, Value::Num(0), "time.wait 0 returns 0");
}

#[test]
fn time_wait_negative_is_error() {
    let v = eval_one("time.wait -1");
    assert_eq!(v, Value::Num(-1), "time.wait negative -> error sentinel");
}
