use super::java_order;

#[test]
fn integer_iteration_matches_java_through_resizes_treeification_and_splits() {
    let mut input = Vec::new();
    let mut output = Vec::new();
    let mut cases = 0;
    for line in include_str!("golden.txt").lines() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("case") => {
                input.clear();
                output.clear();
            }
            Some("key" | "out") => {
                let key: u32 = words
                    .next()
                    .unwrap_or_else(|| unreachable!("valid fixture"))
                    .parse()
                    .unwrap_or_else(|_| unreachable!("valid fixture"));
                let tile = ((key >> 16) as i32, (key & 65535) as i32);
                if line.starts_with("key") {
                    input.push(tile);
                } else {
                    output.push(tile);
                }
            }
            Some("end") => {
                assert_eq!(java_order(input.clone()), output, "case {cases}");
                cases += 1;
            }
            _ => panic!("invalid oracle line"),
        }
    }
    assert_eq!(cases, 6);
}
