pub fn bf_to_wasm(
    bf: &str,
    name: Option<&str>,
    pages: u32,
    custom_section: Option<(String, Vec<u8>)>,
) -> Result<Vec<u8>, BfError> {
    let bf = Bf::parse(bf)?;

    let mut module = wabam::Module::EMPTY;

    module.custom_sections.push(wabam::NameSection {
        module_name: name.map(str::to_owned),
        function_names: vec![
            (0, "fd_write".into()),
            (1, "fd_read".into()),
            (2, "_start".into()),
        ],
        local_names: vec![
            (2, vec![
                (0, "pointer".into())
            ])
        ],
    }.to_custom());

    if let Some((name, data)) = custom_section {
        module
            .custom_sections
            .push(wabam::CustomSection { name, data })
    }

    module.types = vec![
        wabam::func_type!(),
        wabam::func_type!((param i32 i32 i32 i32) (result i32)),
    ];

    module.imports = vec![
        wabam::Import {
            module: "wasi_snapshot_preview1".into(),
            name: "fd_write".into(),
            desc: wabam::ImportDescription::Func { type_idx: 1 },
        },
        wabam::Import {
            module: "wasi_snapshot_preview1".into(),
            name: "fd_read".into(),
            desc: wabam::ImportDescription::Func { type_idx: 1 },
        },
    ];

    module.memories = vec![wabam::Limit {
        start: pages,
        end: None,
    }];

    module.exports = vec![
        wabam::Export {
            name: "_start".into(),
            desc: wabam::ExportDesc::Func(2),
        },
        wabam::Export {
            name: "memory".into(),
            desc: wabam::ExportDesc::Memory(0),
        },
    ];

    use wabam::Instruction::*;
    let mut body = vec![
        I32Const(0),
        I32Const(1),
        I32Store {
            align: 2,
            offset: 4,
        },
    ];

    for c in bf.0 {
        match c {
            BfInstr::Right(x) => {
                body.extend([
                    LocalGet(0),
                    I32Const(x),
                    I32Add,
                    LocalSet(0),
                ]);
            }
            BfInstr::Add(x) => {
                body.extend([
                    LocalGet(0),
                    LocalGet(0),
                    I32LoadS8 {
                        align: 0,
                        offset: 12,
                    },
                    I32Const(x as i32),
                    I32Add,
                    I32StoreI8 {
                        align: 0,
                        offset: 12,
                    },
                ]);
            }
            BfInstr::Output => {
                body.extend([
                    I32Const(0),
                    LocalGet(0),
                    I32Const(12),
                    I32Add,
                    I32Store {
                        align: 2,
                        offset: 0,
                    },
                    I32Const(1),
                    I32Const(0),
                    I32Const(1),
                    I32Const(8),
                    Call(0),
                    If(None),
                    Unreachable,
                    End,
                ]);
            }
            BfInstr::Input => {
                body.extend([
                    I32Const(0),
                    LocalGet(0),
                    I32Const(12),
                    I32Add,
                    I32Store {
                        align: 2,
                        offset: 0,
                    },
                    I32Const(0),
                    I32Const(0),
                    I32Const(1),
                    I32Const(8),
                    Call(1),
                    If(None),
                    Unreachable,
                    End,
                ]);
            }
            BfInstr::StartLoop => {
                body.extend([
                    LocalGet(0),
                    I32LoadS8 {
                        align: 0,
                        offset: 12,
                    },
                    If(None),
                    Loop(None),
                ]);
            }
            BfInstr::EndLoop => {
                body.extend([
                    LocalGet(0),
                    I32LoadS8 {
                        align: 0,
                        offset: 12,
                    },
                    BranchIf(0),
                    End,
                    End,
                ]);
            }
        }
    }

    module.functions = vec![wabam::Function {
        type_idx: 0,
        locals: vec![wabam::ValType::I32],
        body: wabam::Expr { instructions: body },
    }];

    Ok(module.build())
}


#[derive(Debug)]
struct Bf(Vec<BfInstr>);

impl Bf {
    pub fn parse(raw: &str) -> Result<Self, BfError> {
        let mut ast = Vec::new();
        let mut brackets = 0;
        for c in raw.chars().filter(is_valid_instr) {
            match c {
                '+' => {
                    if let Some(BfInstr::Add(x)) = ast.last_mut() {
                        *x = x.wrapping_add(1);
                    } else {
                        ast.push(BfInstr::Add(1));
                    }
                }
                '-' => {
                    if let Some(BfInstr::Add(x)) = ast.last_mut() {
                        *x = x.wrapping_sub(1);
                    } else {
                        ast.push(BfInstr::Add(-1));
                    }
                }
                '>' => {
                    if let Some(BfInstr::Right(x)) = ast.last_mut() {
                        *x = x.wrapping_add(1);
                    } else {
                        ast.push(BfInstr::Right(1));
                    }
                }
                '<' => {
                    if let Some(BfInstr::Right(x)) = ast.last_mut() {
                        *x = x.wrapping_sub(1);
                    } else {
                        ast.push(BfInstr::Right(-1));
                    }
                }
                '.' => ast.push(BfInstr::Output),
                ',' => ast.push(BfInstr::Input),
                '[' => {
                    brackets += 1;
                    ast.push(BfInstr::StartLoop)
                }
                ']' => {
                    brackets -= 1;
                    ast.push(BfInstr::EndLoop)
                }
                _ => (/* nope */),
            };

            if matches!(ast.last(), Some(BfInstr::Add(0)) | Some(BfInstr::Right(0))) {
                let _ = ast.pop().unwrap();
            }
        }

        if brackets != 0 {
            return Err(BfError::BracketMismatch);
        }
        Ok(Self(ast))
    }
}

fn is_valid_instr(c: &char) -> bool {
    matches!(c, '+' | '-' | '<' | '>' | '.' | ',' | '[' | ']')
}

#[derive(Debug)]
pub enum BfError {
    BracketMismatch,
}

impl std::error::Error for BfError {}
impl std::fmt::Display for BfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BfError::BracketMismatch => f.pad("bracket mismatch"),
        }
    }
}

#[derive(Debug)]
enum BfInstr {
    Add(i8), // sub is just this but negative
    Right(i32), // left is just this but negative
    Output,
    Input,
    StartLoop,
    EndLoop,
}
