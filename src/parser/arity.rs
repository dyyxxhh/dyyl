//! Known command arities (fixed parameter counts).
//!
//! Commands with known arity participate in greedy-RHS parsing.
//! Unknown arity commands get variable-arity treatment (all params
//! are individual tokens, no greedy merging).

/// Return the fixed arity for `command`, or `None` if unknown.
pub(super) fn known_arity(command: &str) -> Option<usize> {
    let arity = match command {
        "set" | "create.num" | "create.str" => 2,
        "language" => 1,
        "math.add" | "math.sub" | "math.multi" | "math.div" => 2,
        "math.pow" => 2,
        "math.sqrt" | "math.abs" => 1,
        "math.strike" | "math.surplus" => 2,
        "math.round" | "math.floor" | "math.ceil" => 1,
        "math.sin" | "math.cos" | "math.tan" => 1,
        "math.asin" | "math.acos" | "math.atan" => 1,
        "math.ln" | "math.lg" | "math.exp" => 1,
        "math.log" => 2,
        "math.approx" => 1,
        "math.hash" => 2,
        "math.pi" | "math.e" | "math.tau" => 0,
        "logic.or" | "logic.and" | "logic.same" => 2,
        "logic.not.same" | "logic.more" | "logic.less" => 2,
        "logic.more.same" | "logic.less.same" => 2,
        "logic.max" | "logic.min" => 2,
        "logic.between" | "logic.clamp" => 3,
        "logic.un" | "logic.is.num" | "logic.is.str" | "logic.is.empty" => 1,
        "logic.if" | "logic.else" | "logic.while" | "logic.for" => 2,
        "io.out" | "io.changeline" => 1,
        "io.in" | "io.get" => 0,
        "io.inpasswd" => 1,
        "dict.create" | "list.create" => 1,
        "dict.get" | "dict.has" | "dict.del" => 2,
        "dict.set" => 3,
        "dict.keys" | "dict.vals" | "dict.len" => 1,
        "list.get" | "list.len" | "list.append" | "list.contains" | "list.index" => 2,
        "list.insert" | "list.slice" => 3,
        "list.remove" | "list.join" => 2,
        "list.reverse" | "list.sort" => 1,

        // str.* commands
        "str.len" | "str.upper" | "str.lower" | "str.capital" | "str.reverse" => 1,
        "str.trim" | "str.trim.left" | "str.trim.right" => 1,
        "str.escape" | "str.unescape" => 1,
        "str.to.num" | "str.from.num" => 1,
        "str.get" | "str.find" | "str.rfind" | "str.count" => 2,
        "str.repeat" | "str.start" | "str.end" | "str.contains" | "str.index" => 2,
        "str.split" | "str.join" => 2,
        "str.match" | "str.extract" => 2,
        "str.encode" | "str.decode" => 2,
        "str.slice" | "str.insert" | "str.remove" => 3,
        "str.replace" | "str.replace.all" | "str.replace.regex" => 3,
        "str.pad.left" | "str.pad.right" => 3,
        "str.format" => 2,

        // file.* commands
        "file.write" | "file.append" => 2,
        "file.read" => 1,

        // net.* commands
        "net.get" => 1,
        "net.download" => 2,

        // user.* commands
        "user.id" | "user.name" => 0,
        "user.bash" => 1,

        // system.* commands
        "system.os" | "system.arch" => 0,

        // cli.* commands
        "cli.args" | "cli.count" => 0,
        "cli.get" => 1,

        // time.* commands
        "time.get" | "time.now" | "time.year" | "time.month" | "time.day" => 0,
        "time.hour" | "time.minute" | "time.second" => 0,
        "time.weekday" | "time.weekday.name" => 0,
        "time.format" => 1,
        "time.diff" => 2,
        "time.add" => 2,
        "time.wait" => 1,

        // ai.* commands
        // ai.ask: [system], <prompt> — registered as 2 (max); the handler
        // treats a single arg as "use default system prompt".
        "ai.ask" => 2,
        // ai.auto.filled: <hint>, <value> — exactly 2 args (hint ignored at
        // runtime, value returned verbatim).
        "ai.auto.filled" => 2,

        _ => return None,
    };
    Some(arity)
}
