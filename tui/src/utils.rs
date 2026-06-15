use jf_import_library::media::tree;

pub fn recursive_print(node: &tree::TreeNode, old_indent: String, print_failures: bool) {
    // `tree` ripoff
    const connector: &'static str = "│   ";
    const middle:    &'static str = "├── ";
    const end:       &'static str = "└── ";
    const empty:     &'static str = "    ";

    print!("{}", old_indent);
    // println!("{node}");

    let mut next_indent = if old_indent.chars().count() > 3 {
        let split_point = old_indent.char_indices().rev().nth(3).map_or(0, |(idx, _)| idx);
        let (rest, last) = old_indent.split_at(split_point);
        
        match &last.chars().nth(0).unwrap() {
            '├' => String::from(rest) + connector,
            '└' => String::from(rest) + empty,
             _  => unreachable!()
        }
    }
    else { String::new() };

    match node {
        tree::TreeNode::Item{ inner, children } => {
            for child in children {
                let mut copy = next_indent.clone();
                let last = children.last().unwrap();
                // ref: https://users.rust-lang.org/t/is-any-way-to-know-references-are-referencing-the-same-object/9716/6
                if child as *const _ != children.last().unwrap() as *const _ 
                     { copy += middle; }
                else { copy += end; }
                recursive_print(child, copy.clone(), print_failures);
            }
        }
        tree::TreeNode::Category { name, children } => {
            if name == "Failures" && !print_failures { return; }
            for child in children {
                let mut copy = next_indent.clone();
                let last = children.last().unwrap();
                // ref: https://users.rust-lang.org/t/is-any-way-to-know-references-are-referencing-the-same-object/9716/6
                if child as *const _ != children.last().unwrap() as *const _ 
                     { copy += middle; }
                else { copy += end; }
                recursive_print(child, copy.clone(), print_failures);
            }
        }
        _ => {}
    }
}
