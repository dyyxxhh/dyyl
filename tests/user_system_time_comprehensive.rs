use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

fn eval_values(source: &str) -> Vec<Value> {
    run_script(source, false).values
}

#[test]
fn user_system_time_all_commands() {
    let values = eval_values(
        "user.id\n\
         user.name\n\
         user.bash printf dyyl-ok\n\
         user.bash false\n\
         system.os\n\
         system.arch\n\
         time.get\n\
         time.now\n\
         time.year\n\
         time.month\n\
         time.day\n\
         time.hour\n\
         time.minute\n\
         time.second\n\
         time.weekday\n\
         time.weekday.name\n\
         time.format YYYY\n\
         time.diff 0, 86400\n\
         time.add 0, 86400",
    );

    assert_eq!(values.len(), 19, "expected 19 return values");

    match &values[0] {
        Value::Str(s) => assert!(!s.is_empty()),
        other => panic!("user.id: expected Str, got {other:?}"),
    }

    match &values[1] {
        Value::Str(s) => assert!(!s.is_empty()),
        other => panic!("user.name: expected Str, got {other:?}"),
    }

    assert_eq!(values[2], Value::Str("dyyl-ok".into()));
    assert_eq!(values[3], Value::Num(-1));

    match &values[4] {
        Value::Str(s) => assert!(!s.is_empty()),
        other => panic!("system.os: expected Str, got {other:?}"),
    }

    match &values[5] {
        Value::Str(s) => assert!(!s.is_empty()),
        other => panic!("system.arch: expected Str, got {other:?}"),
    }

    match &values[6] {
        Value::Num(n) => assert!(*n > 1_000_000_000),
        other => panic!("time.get: expected Num, got {other:?}"),
    }

    match &values[7] {
        Value::Str(s) => {
            assert_eq!(s.len(), 19);
            assert_eq!(&s[4..5], "-");
            assert_eq!(&s[7..8], "-");
            assert_eq!(&s[10..11], " ");
            assert_eq!(&s[13..14], ":");
            assert_eq!(&s[16..17], ":");
        }
        other => panic!("time.now: expected Str, got {other:?}"),
    }

    match &values[8] {
        Value::Num(n) => assert!(*n >= 2024 && *n <= 2100),
        other => panic!("time.year: expected Num, got {other:?}"),
    }

    match &values[9] {
        Value::Num(n) => assert!(*n >= 1 && *n <= 12),
        other => panic!("time.month: expected Num, got {other:?}"),
    }

    match &values[10] {
        Value::Num(n) => assert!(*n >= 1 && *n <= 31),
        other => panic!("time.day: expected Num, got {other:?}"),
    }

    match &values[11] {
        Value::Num(n) => assert!(*n >= 0 && *n <= 23),
        other => panic!("time.hour: expected Num, got {other:?}"),
    }

    match &values[12] {
        Value::Num(n) => assert!(*n >= 0 && *n <= 59),
        other => panic!("time.minute: expected Num, got {other:?}"),
    }

    match &values[13] {
        Value::Num(n) => assert!(*n >= 0 && *n <= 59),
        other => panic!("time.second: expected Num, got {other:?}"),
    }

    match &values[14] {
        Value::Num(n) => assert!(*n >= 1 && *n <= 7),
        other => panic!("time.weekday: expected Num, got {other:?}"),
    }

    match &values[15] {
        Value::Str(s) => assert!(!s.is_empty()),
        other => panic!("time.weekday.name: expected Str, got {other:?}"),
    }

    match &values[16] {
        Value::Str(s) => assert_eq!(s.len(), 4),
        other => panic!("time.format YYYY: expected Str, got {other:?}"),
    }

    assert_eq!(values[17], Value::Num(86400));
    assert_eq!(values[18], Value::Num(86400));
}
