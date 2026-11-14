use md_hlsl::{tokenize, TokKind, TokKind::*};

fn kinds(s: &str) -> Vec<TokKind> {
    tokenize(s).unwrap().into_iter().map(|t| t.kind).collect()
}

#[test]
fn lexes_float_with_f_suffix() {
    assert_eq!(kinds("1.f"), vec![Float(1.0)]);
    assert_eq!(
        kinds("1.0F 1.0h .5 1e-3 2. 1.5e+2f"),
        vec![
            Float(1.0),
            Float(1.0),
            Float(0.5),
            Float(1e-3),
            Float(2.0),
            Float(150.0)
        ]
    );
    assert_eq!(
        kinds("1 0x1F 1u 010 0"),
        vec![Int(1), Int(31), Int(1), Int(8), Int(0)]
    );
    // `.xyz` after an identifier is a swizzle, not a number.
    assert_eq!(
        kinds("a.x"),
        vec![Ident("a".into()), Op("."), Ident("x".into())]
    );
    assert_eq!(kinds("1.0f.x").len(), 3);
}

#[test]
fn lexes_operators_longest_match() {
    assert_eq!(
        kinds("a<<=b>>c?d:e++ --f+=1 x!=y&&z||w"),
        vec![
            Ident("a".into()),
            Op("<<="),
            Ident("b".into()),
            Op(">>"),
            Ident("c".into()),
            Op("?"),
            Ident("d".into()),
            Op(":"),
            Ident("e".into()),
            Op("++"),
            Op("--"),
            Ident("f".into()),
            Op("+="),
            Int(1),
            Ident("x".into()),
            Op("!="),
            Ident("y".into()),
            Op("&&"),
            Ident("z".into()),
            Op("||"),
            Ident("w".into()),
        ]
    );
    assert_eq!(
        kinds("{}()[];,."),
        "{}()[];,."
            .chars()
            .map(|c| Op(match c {
                '{' => "{",
                '}' => "}",
                '(' => "(",
                ')' => ")",
                '[' => "[",
                ']' => "]",
                ';' => ";",
                ',' => ",",
                _ => ".",
            }))
            .collect::<Vec<_>>()
    );
}

#[test]
fn strips_comments() {
    assert_eq!(
        kinds("a // x\n/* y\n z */ b"),
        vec![Ident("a".into()), Ident("b".into())]
    );
}

#[test]
fn reports_line_and_col() {
    let t = tokenize("float x;\n  /* c\n */ y = 1.5;").unwrap();
    let pos: Vec<_> = t.iter().map(|t| (t.line, t.col)).collect();
    assert_eq!(
        pos,
        vec![(1, 1), (1, 7), (1, 8), (3, 5), (3, 7), (3, 9), (3, 12)]
    );
    let e = tokenize("a\n  $").unwrap_err();
    assert_eq!((e.line, e.col), (2, 3));
    let e = tokenize("x /* open").unwrap_err();
    assert_eq!((e.line, e.col), (1, 3));
    let e = tokenize("1.0x").unwrap_err();
    assert_eq!((e.line, e.col), (1, 1));
}
