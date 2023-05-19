pub fn bf_to_wasm(
    bf: &str,
    name: Option<&str>,
    pages: u32,
    custom_section: Option<(String, Vec<u8>)>,
) -> Result<Vec<u8>, BfError> {
    let bf = Bf::parse(bf)?;

    let mut module = wabam::Module::EMPTY;

    module.custom_sections.push(wabam::customs::NameSection {
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
            .push(wabam::customs::CustomSection { name, data })
    }

    module.types = vec![
        wabam::func_type!(),
        wabam::func_type!((param i32 i32 i32 i32) (result i32)),
    ];

    module.imports = vec![
        wabam::interface::Import {
            module: "wasi_snapshot_preview1".into(),
            name: "fd_write".into(),
            desc: wabam::interface::ImportDesc::Func { type_idx: 1 },
        },
        wabam::interface::Import {
            module: "wasi_snapshot_preview1".into(),
            name: "fd_read".into(),
            desc: wabam::interface::ImportDesc::Func { type_idx: 1 },
        },
    ];

    module.memories = vec![wabam::Limit {
        start: pages,
        end: None,
    }];

    let mut body = Vec::new();
    
    body.extend(wabam::instrs!(
        (i32.const 0)
        (i32.const 1)
        (i32.store offset=4)
    ));

    for c in bf.0 {
        match c {
            BfInstr::Right(x) => {
                body.extend(wabam::instrs!(
                    (local.get 0)
                    (i32.const { x })
                    (i32.add)
                    (local.set 0)
                ));
            }
            BfInstr::Add(x) => {
                body.extend(wabam::instrs!(
                    (local.get 0)
                    (local.get 0)
                    (i32.load8_s offset=12)
                    (i32.const { x as i32 })
                    (i32.add)
                    (i32.store8 offset=12)
                ));
            }
            BfInstr::Output => {
                body.extend(wabam::instrs!(
                    (i32.const 0)
                    (local.get 0)
                    (i32.const 12)
                    (i32.add)
                    (i32.store)
                    (i32.const 1)
                    (i32.const 0)
                    (i32.const 1)
                    (i32.const 8)
                    (call 0)
                    (if)
                    (unreachable)
                    (end)
                ));
            }
            BfInstr::Input => {
                body.extend(wabam::instrs!(
                    (i32.const 0)
                    (local.get 0)
                    (i32.const 12)
                    (i32.add)
                    (i32.store)
                    (i32.const 0)
                    (i32.const 0)
                    (i32.const 1)
                    (i32.const 8)
                    (call 1)
                    (if)
                    (unreachable)
                    (end)
                ));
            }
            BfInstr::StartLoop => {
                body.extend(wabam::instrs!(
                    (local.get 0)
                    (i32.load8_s offset=12)
                    (if)
                    (loop)
                ));
            }
            BfInstr::EndLoop => {
                body.extend(wabam::instrs!(
                    (local.get 0)
                    (i32.load8_s offset=12)
                    (br_if 0)
                    (end)
                    (end)
                ));
            }
        }
    }

    module.functions = vec![wabam::functions::Function {
        type_idx: 0,
        locals: vec![wabam::ValType::I32],
        body: body.into(),
    }];

    module.exports = vec![
        wabam::interface::Export {
            name: "_start".into(),
            desc: wabam::interface::ExportDesc::Func { func_idx: 2 },
        },
        wabam::interface::Export {
            name: "memory".into(),
            desc: wabam::interface::ExportDesc::Memory { mem_idx: 0 },
        },
    ];

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
