use railroad::*;

fn boxed<T: Node + 'static>(node: T) -> Box<dyn Node> {
    Box::new(node)
}

fn term(text: &str) -> Terminal {
    Terminal::new(text.to_owned())
}

fn nonterm(text: &str) -> NonTerminal {
    NonTerminal::new(text.to_owned())
}

fn comment(text: &str) -> Comment {
    Comment::new(text.to_owned())
}

pub(crate) fn simple_sequence_diagram() -> Diagram<Sequence<Box<dyn Node>>> {
    Diagram::with_default_css(Sequence::new(vec![
        boxed(SimpleStart),
        boxed(term("BEGIN")),
        boxed(nonterm("syntax")),
        boxed(SimpleEnd),
    ]))
}

pub(crate) fn vertical_grid_diagram() -> Diagram<VerticalGrid<Box<dyn Node>>> {
    Diagram::with_default_css(VerticalGrid::new(vec![
        boxed(Sequence::new(vec![
            boxed(SimpleStart),
            boxed(term("42")),
            boxed(SimpleEnd),
        ])),
        boxed(comment("This is the answer")),
        boxed(Sequence::new(vec![
            boxed(Start),
            boxed(Debug::new(25, 35, 20)),
            boxed(End),
        ])),
    ]))
}

pub(crate) fn column_constraint_diagram() -> Diagram<Sequence<Box<dyn Node>>> {
    let conflict_clause = || nonterm("conflict-clause");

    Diagram::with_default_css(Sequence::new(vec![
        boxed(SimpleStart),
        boxed(Optional::new(Sequence::new(vec![
            boxed(term("CONSTRAINT")),
            boxed(nonterm("name")),
        ]))),
        boxed(Choice::new(vec![
            boxed(Sequence::new(vec![
                boxed(term("PRIMARY")),
                boxed(term("KEY")),
                boxed(Optional::new(Choice::new(vec![
                    boxed(term("ASC")),
                    boxed(term("DESC")),
                ]))),
                boxed(conflict_clause()),
                boxed(Optional::new(term("AUTOINCREMENT"))),
            ])),
            boxed(Sequence::new(vec![
                boxed(term("NOT")),
                boxed(term("NULL")),
                boxed(conflict_clause()),
            ])),
            boxed(Sequence::new(vec![
                boxed(term("UNIQUE")),
                boxed(conflict_clause()),
            ])),
            boxed(Sequence::new(vec![
                boxed(term("CHECK")),
                boxed(term("(")),
                boxed(nonterm("expr")),
                boxed(term(")")),
            ])),
            boxed(Sequence::new(vec![
                boxed(term("DEFAULT")),
                boxed(Choice::new(vec![
                    boxed(nonterm("signed-number")),
                    boxed(nonterm("literal-value")),
                    boxed(Sequence::new(vec![
                        boxed(term("(")),
                        boxed(nonterm("expr")),
                        boxed(term(")")),
                    ])),
                ])),
            ])),
            boxed(Sequence::new(vec![
                boxed(term("COLLATE")),
                boxed(nonterm("collation-name")),
            ])),
            boxed(nonterm("foreign-key-clause")),
        ])),
        boxed(SimpleEnd),
    ]))
}

pub(crate) fn create_table_stmt_diagram() -> Diagram<Sequence<Box<dyn Node>>> {
    let row1: Box<dyn Node> = boxed(Sequence::new(vec![
        boxed(comment("create-table-stmt")),
        boxed(term("CREATE")),
        boxed(LabeledBox::new(
            Optional::new(Choice::new(vec![
                boxed(term("TEMP")),
                boxed(term("TEMPORARY")),
            ])),
            comment("Table will be dropped when connection closes"),
        )),
        boxed(term("TABLE")),
    ]));

    let row2: Box<dyn Node> = boxed(Sequence::new(vec![
        boxed(LabeledBox::new(
            Optional::new(Sequence::new(vec![
                boxed(term("IF")),
                boxed(term("NOT")),
                boxed(term("EXISTS")),
            ])),
            comment("If table exists, do nothing"),
        )),
        boxed(LabeledBox::new(
            Sequence::new(vec![
                boxed(Optional::new(LabeledBox::new(
                    Sequence::new(vec![boxed(nonterm("schema-name")), boxed(term("."))]),
                    comment("...in a foreign database"),
                ))),
                boxed(nonterm("table-name")),
            ]),
            comment("The table's name"),
        )),
    ]));

    let row3: Box<dyn Node> = boxed(Choice::new(vec![
        boxed(Sequence::new(vec![
            boxed(term("(")),
            boxed(LabeledBox::new(
                Repeat::new(nonterm("column-def"), term(",")),
                comment("One or more column-definitions"),
            )),
            boxed(LabeledBox::new(
                Optional::new(LabeledBox::new(
                    Repeat::new(nonterm("table-constraint"), term(",")),
                    comment("primary key and stuff"),
                )),
                comment("Zero or more table-constraints"),
            )),
            boxed(term(")")),
            boxed(Optional::new(Sequence::new(vec![
                boxed(term("WITHOUT")),
                boxed(term("ROWID")),
            ]))),
        ])),
        boxed(LabeledBox::new(
            Sequence::new(vec![boxed(term("AS")), boxed(nonterm("select-stmt"))]),
            comment("Create table definition and content directly from a query"),
        )),
    ]));

    Diagram::with_default_css(Sequence::new(vec![
        boxed(SimpleStart),
        boxed(Stack::new(vec![row1, row2, row3])),
        boxed(SimpleEnd),
    ]))
}
