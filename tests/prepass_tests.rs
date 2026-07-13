use dyyl::prepass::scan_placeholders;

#[test]
fn scan_finds_ai_auto_without_hint() {
    let content = "set $x, ai.auto\nio.out $x\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].line, 1);
    assert!(phs[0].hint.is_none());
    assert_eq!(phs[0].original_text, "ai.auto");
}

#[test]
fn scan_finds_ai_auto_with_quoted_hint() {
    let content = "set $port, ai.auto \"端口常用25565\"\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].hint.as_deref(), Some("端口常用25565"));
}

#[test]
fn scan_finds_ai_auto_with_bareword_hint() {
    let content = "set $x, ai.auto some_hint\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].hint.as_deref(), Some("some_hint"));
}

#[test]
fn scan_finds_multiple_placeholders() {
    let content = "set $a, ai.auto \"first\"\nset $b, ai.auto\nset $c, ai.auto \"third\"\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 3);
    assert_eq!(phs[0].id, 1);
    assert_eq!(phs[1].id, 2);
    assert_eq!(phs[2].id, 3);
}

#[test]
fn scan_ignores_ai_auto_filled() {
    let content = "set $x, ai.auto.filled _, \"value\"\nset $y, ai.auto\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1, "ai.auto.filled should not be scanned");
    assert_eq!(phs[0].line, 2);
}

#[test]
fn scan_finds_inline_placeholder() {
    let content = "file.write ai.auto \"路径\", \"content\"\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].hint.as_deref(), Some("路径"));
}

#[test]
fn scan_returns_empty_when_no_placeholders() {
    let content = "io.out hello\nset $x, 42\n";
    let phs = scan_placeholders(content);
    assert!(phs.is_empty());
}
