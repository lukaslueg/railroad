use std::fs;
use std::io::Write;

use railroad::*;

fn column_constraint() -> impl Node {
    let conflict_clause = || NonTerminal::new("conflict-clause".to_owned());

    Sequence::new(vec![
        Box::new(SimpleStart) as Box<dyn Node>,
        Box::new(Optional::new(Sequence::new(vec![
            Box::new(Terminal::new("CONSTRAINT".to_owned())) as Box<dyn Node>,
            Box::new(NonTerminal::new("name".to_owned())),
        ]))),
        Box::new(Choice::new(vec![
            Box::new(Sequence::new(vec![
                Box::new(Terminal::new("PRIMARY".to_owned())) as Box<dyn Node>,
                Box::new(Terminal::new("KEY".to_owned())),
                Box::new(Optional::new(Choice::new(vec![
                    Box::new(Terminal::new("ASC".to_owned())) as Box<dyn Node>,
                    Box::new(Terminal::new("DESC".to_owned())),
                ]))),
                Box::new(conflict_clause()),
                Box::new(Optional::new(Terminal::new("AUTOINCREMENT".to_owned()))),
            ])) as Box<dyn Node>,
            Box::new(Sequence::new(vec![
                Box::new(Terminal::new("NOT".to_owned())) as Box<dyn Node>,
                Box::new(Terminal::new("NULL".to_owned())),
                Box::new(conflict_clause()),
            ])),
            Box::new(Sequence::new(vec![
                Box::new(Terminal::new("UNIQUE".to_owned())) as Box<dyn Node>,
                Box::new(conflict_clause()),
            ])),
            Box::new(Sequence::new(vec![
                Box::new(Terminal::new("CHECK".to_owned())) as Box<dyn Node>,
                Box::new(Terminal::new("(".to_owned())),
                Box::new(NonTerminal::new("expr".to_owned())),
                Box::new(Terminal::new(")".to_owned())),
            ])),
            Box::new(Sequence::new(vec![
                Box::new(Terminal::new("DEFAULT".to_owned())) as Box<dyn Node>,
                Box::new(Choice::new(vec![
                    Box::new(NonTerminal::new("signed-number".to_owned())) as Box<dyn Node>,
                    Box::new(NonTerminal::new("literal-value".to_owned())),
                    Box::new(Sequence::new(vec![
                        Box::new(Terminal::new("(".to_owned())) as Box<dyn Node>,
                        Box::new(NonTerminal::new("expr".to_owned())),
                        Box::new(Terminal::new(")".to_owned())),
                    ])),
                ])),
            ])),
            Box::new(Sequence::new(vec![
                Box::new(Terminal::new("COLLATE".to_owned())) as Box<dyn Node>,
                Box::new(NonTerminal::new("collation-name".to_owned())),
            ])),
            Box::new(NonTerminal::new("foreign-key-clause".to_owned())),
        ])) as Box<dyn Node>,
        Box::new(SimpleEnd),
    ])
}

fn create_table_stmt() -> impl Node {
    let row1: Box<dyn Node> = Box::new(Sequence::new(vec![
        Box::new(Comment::new("create-table-stmt".to_owned())) as Box<dyn Node>,
        Box::new(Terminal::new("CREATE".to_owned())),
        Box::new(LabeledBox::new(
            Optional::new(Choice::new(vec![
                Box::new(Terminal::new("TEMP".to_owned())) as Box<dyn Node>,
                Box::new(Terminal::new("TEMPORARY".to_owned())),
            ])),
            Comment::new("Table will be dropped when connection closes".to_owned()),
        )),
        Box::new(Terminal::new("TABLE".to_owned())),
    ]));

    let row2: Box<dyn Node> = Box::new(Sequence::new(vec![
        Box::new(LabeledBox::new(
            Optional::new(Sequence::new(vec![
                Box::new(Terminal::new("IF".to_owned())) as Box<dyn Node>,
                Box::new(Terminal::new("NOT".to_owned())),
                Box::new(Terminal::new("EXISTS".to_owned())),
            ])),
            Comment::new("If table exists, do nothing".to_owned()),
        )) as Box<dyn Node>,
        Box::new(LabeledBox::new(
            Sequence::new(vec![
                Box::new(Optional::new(LabeledBox::new(
                    Sequence::new(vec![
                        Box::new(NonTerminal::new("schema-name".to_owned())) as Box<dyn Node>,
                        Box::new(Terminal::new(".".to_owned())),
                    ]),
                    Comment::new("...in a foreign database".to_owned()),
                ))) as Box<dyn Node>,
                Box::new(NonTerminal::new("table-name".to_owned())),
            ]),
            Comment::new("The table's name".to_owned()),
        )),
    ]));

    let row3: Box<dyn Node> = Box::new(Choice::new(vec![
        Box::new(Sequence::new(vec![
            Box::new(Terminal::new("(".to_owned())) as Box<dyn Node>,
            Box::new(LabeledBox::new(
                Repeat::new(
                    NonTerminal::new("column-def".to_owned()),
                    Terminal::new(",".to_owned()),
                ),
                Comment::new("One or more column-definitions".to_owned()),
            )),
            Box::new(LabeledBox::new(
                Optional::new(LabeledBox::new(
                    Repeat::new(
                        NonTerminal::new("table-constraint".to_owned()),
                        Terminal::new(",".to_owned()),
                    ),
                    Comment::new("primary key and stuff".to_owned()),
                )),
                Comment::new("Zero or more table-constraints".to_owned()),
            )),
            Box::new(Terminal::new(")".to_owned())),
            Box::new(Optional::new(Sequence::new(vec![
                Box::new(Terminal::new("WITHOUT".to_owned())) as Box<dyn Node>,
                Box::new(Terminal::new("ROWID".to_owned())),
            ]))),
        ])) as Box<dyn Node>,
        Box::new(LabeledBox::new(
            Sequence::new(vec![
                Box::new(Terminal::new("AS".to_owned())) as Box<dyn Node>,
                Box::new(NonTerminal::new("select-stmt".to_owned())),
            ]),
            Comment::new("Create table definition and content directly from a query".to_owned()),
        )),
    ]));

    Sequence::new(vec![
        Box::new(SimpleStart) as Box<dyn Node>,
        Box::new(Stack::new(vec![row1, row2, row3])),
        Box::new(SimpleEnd),
    ])
}

fn main() {
    let mut seq = Sequence::default();
    seq.push(Box::new(Start) as Box<dyn Node>)
        .push(Box::new(Terminal::new("BEGIN".to_owned())))
        .push(Box::new(NonTerminal::new("syntax".to_owned())))
        .push(Box::new(End));
    let dia = Diagram::new_with_stylesheet(seq, &Stylesheet::LightRendersafe);
    let png_buffer = render::to_png(&dia.to_string(), &render::FitTo::default()).unwrap();
    let mut f = fs::File::create("examples/render.png").unwrap();
    f.write_all(&png_buffer).unwrap();

    let dia = Diagram::new_with_stylesheet(column_constraint(), &Stylesheet::LightRendersafe);
    let png_buffer = render::to_png(&dia.to_string(), &render::FitTo::MaxWidth(1800)).unwrap();
    let mut f = fs::File::create("examples/column_constraint.png").unwrap();
    f.write_all(&png_buffer).unwrap();

    let dia = Diagram::new_with_stylesheet(create_table_stmt(), &Stylesheet::LightRendersafe);
    let png_buffer = render::to_png(&dia.to_string(), &render::FitTo::MaxWidth(1800)).unwrap();
    let mut f = fs::File::create("examples/create_table_stmt.png").unwrap();
    f.write_all(&png_buffer).unwrap();
}
