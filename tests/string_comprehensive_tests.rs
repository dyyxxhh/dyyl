use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

fn eval_values(source: &str) -> Vec<String> {
    let out = run_script(source, false);
    out.values.iter().map(Value::to_string).collect()
}

#[test]
fn string_all_commands_unicode_and_regex() {
    let source = "str.len \"hello\"\n\
                   str.len \"你好世界\"\n\
                   str.len \"👍🎉\"\n\
                   str.get \"hello\", 1\n\
                   str.get \"你好\", 1\n\
                   str.slice \"hello\", 1, 3\n\
                   str.slice \"你好世界\", 1, 3\n\
                   str.find \"hello world\", \"world\"\n\
                   str.rfind \"aabaa\", \"aa\"\n\
                   str.count \"aabaa\", \"aa\"\n\
                   str.replace \"aabaa\", \"aa\", \"x\"\n\
                   str.replace.all \"aabaa\", \"aa\", \"x\"\n\
                   str.insert \"hello\", 1, \"XY\"\n\
                   str.remove \"hello\", 1, 3\n\
                   str.upper \"hello\"\n\
                   str.lower \"HELLO\"\n\
                   str.capital \"hello\"\n\
                   str.reverse \"hello\"\n\
                   str.reverse \"你好世界\"\n\
                   str.repeat \"ab\", 3\n\
                   str.pad.left \"hi\", 5, \".\"\n\
                   str.pad.right \"hi\", 5, \".\"\n\
                   str.trim \"  hello  \"\n\
                   str.trim.left \"  hello  \"\n\
                   str.trim.right \"  hello  \"\n\
                   str.split \"a,,b\", \",\"\n\
                   str.start \"hello\", \"hel\"\n\
                   str.end \"hello\", \"llo\"\n\
                   str.contains \"hello world\", \"world\"\n\
                   str.index \"hello world\", \"world\"\n\
                   str.match \"hello123\", \"[0-9]+\"\n\
                   str.extract \"hello123world\", \"[0-9]+\"\n\
                   str.replace.regex \"hello123world\", \"[0-9]+\", \"X\"\n\
                   str.escape \"a.b*c\"\n\
                   str.encode \"hello\", \"base64\"\n\
                   str.decode \"aGVsbG8=\", \"base64\"\n\
                   str.encode \"hello\", \"hex\"\n\
                   str.decode \"68656c6c6f\", \"hex\"\n\
                   str.encode \"hello world\", \"url\"\n\
                   str.decode \"hello%20world\", \"url\"\n\
                   str.to.num \"42\"\n\
                   str.to.num \"abc\"\n\
                   str.from.num 42\n\
                   str.match \"abc\", \"(\"";

    let vals = eval_values(source);
    let expected = vec![
        "5",             // 0: len "hello"
        "4",             // 1: len "你好世界"
        "2",             // 2: len "👍🎉"
        "e",             // 3: get "hello", 1
        "好",            // 4: get "你好", 1
        "el",            // 5: slice "hello", 1, 3
        "好世",          // 6: slice "你好世界", 1, 3
        "6",             // 7: find
        "3",             // 8: rfind
        "2",             // 9: count
        "xbaa",          // 10: replace (first only)
        "xbx",           // 11: replace.all
        "hXYello",       // 12: insert
        "hlo",           // 13: remove
        "HELLO",         // 14: upper
        "hello",         // 15: lower
        "Hello",         // 16: capital
        "olleh",         // 17: reverse
        "界世好你",      // 18: reverse CJK
        "ababab",        // 19: repeat
        "...hi",         // 20: pad.left
        "hi...",         // 21: pad.right
        "hello",         // 22: trim
        "hello  ",       // 23: trim.left (leading trimmed, trailing preserved)
        "  hello",       // 24: trim.right (leading preserved, trailing trimmed)
        "[a, , b]",      // 25: split (List display)
        "1",             // 26: start
        "1",             // 27: end
        "1",             // 28: contains
        "6",             // 29: index
        "1",             // 30: match
        "123",           // 31: extract
        "helloXworld",   // 32: replace.regex
        r"a\.b\*c",      // 33: escape
        "aGVsbG8=",      // 34: encode base64
        "hello",         // 35: decode base64
        "68656c6c6f",    // 36: encode hex
        "hello",         // 37: decode hex
        "hello%20world", // 38: encode url
        "hello world",   // 39: decode url
        "42",            // 40: to.num valid
        "-1",            // 41: to.num invalid
        "42",            // 42: from.num
        "",              // 43: match invalid regex → sentinel Str("")
    ];

    assert_eq!(
        vals.len(),
        expected.len(),
        "expected {} results, got {vals:?}",
        expected.len()
    );
    for (i, exp) in expected.iter().enumerate() {
        assert_eq!(&vals[i], exp, "mismatch at index {i}: expected {exp}");
    }
}
