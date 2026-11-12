//! The generated header must declare everything in Nickel's own `nickel_lang.h` (the copy
//! go-nickel vendors, in `tests/upstream/`) exactly as that header does.

use std::path::Path;

fn read(path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// The header's C declarations, without comments, preprocessor lines or the `extern "C"`
/// wrapper, each with its whitespace collapsed.
fn declarations(header: &str) -> Vec<String> {
    let mut code = String::new();
    let mut rest = header;
    while let Some(start) = rest.find("/*") {
        code.push_str(&rest[..start]);
        let end = rest[start..].find("*/").expect("unterminated comment") + start + 2;
        rest = &rest[end..];
    }
    code.push_str(rest);

    let code: String = code
        .lines()
        .map(|line| line.split("//").next().unwrap().trim())
        .filter(|line| !line.starts_with('#') && *line != "extern \"C\" {" && *line != "}")
        .collect::<Vec<_>>()
        .join(" ");

    code.split_inclusive(';')
        .map(|decl| decl.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|decl| !decl.is_empty())
        .collect()
}

#[test]
fn declares_everything_in_nickel_lang_h_unchanged() {
    let ours = declarations(&read("include/nickel_lang.h"));
    let upstream = declarations(&read("tests/upstream/nickel_lang.h"));
    assert!(upstream.len() > 50, "parsed too few upstream declarations");
    let missing: Vec<_> = upstream.iter().filter(|d| !ours.contains(d)).collect();
    assert!(
        missing.is_empty(),
        "declarations missing or changed:\n{missing:#?}"
    );
}

#[test]
fn keeps_the_include_guard() {
    let ours = read("include/nickel_lang.h");
    assert!(ours.contains("#ifndef NICKEL_LANG_H"));
    assert!(ours.contains("#include <stdint.h>"));
}
