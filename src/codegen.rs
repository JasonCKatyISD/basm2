use crate::*;
use colored::*;
pub const HLT_OP: i16 = 0b0000;
pub const ADD_OP: i16 = 0b0001;
pub const BO_OP: i16 = 0b00100;
pub const BNO_OP: i16 = 0b00101;
pub const POP_OP: i16 = 0b0011;
pub const DIV_OP: i16 = 0b0100;
pub const RET_OP: i16 = 0b0101;
pub const BL_OP: i16 = 0b01010;
pub const BG_OP: i16 = 0b01011;
pub const LD_OP: i16 = 0b0110;
pub const ST_OP: i16 = 0b0111;
pub const JMP_OP: i16 = 0b10000;
pub const BZ_OP: i16 = 0b10010;
pub const BNZ_OP: i16 = 0b10011;
pub const CMP_OP: i16 = 0b1010;
pub const NAND_OP: i16 = 0b1011;
pub const PUSH_OP: i16 = 0b1100;
pub const INT_OP: i16 = 0b1101;
pub const MOV_OP: i16 = 0b1110;
pub const LEA_OP: i16 = 0b1111;

const HLT_TYPE: u8 = 0;
const MOV_TYPE: u8 = 1;
const B_TYPE: u8 = 2;
const POP_TYPE: u8 = 3;
const LD_TYPE: u8 = 4;
const ST_TYPE: u8 = 5;
type CodeGenError = ParserError;
use std::ops::Range;

macro_rules! gen_ice { // Generate (I)nternal (C)ompiler (E)rror
    ($($arg:tt)*) => {
        {
            eprintln!(
                "!!! {} !!! [{}] IN {}:{}:{}\nMESSAGE: {} ",
                "FATAL".red().bold(),
                "INTERNAL COMPILER ERROR".red().bold(),
                file!(),
                line!(),
                column!(),
                format!($($arg)*),
            );
            std::process::exit(1);
        }
    };
}

pub fn encode(
    ins: &TokenKind,
    fname: &String,
    next_ins: Option<&(String, TokenKind, Range<usize>)>,
) -> Result<Vec<i16>, CodeGenError> {
    let mut encoded_tokens = Vec::new();
    match ins {
        TokenKind::Instruction(ins) => {
            let (opcode, ins_class) = match ins.name.to_lowercase().as_str() {
                // all instructions should be valid when this is reached, as it is validated in
                // validator/validator_ins.rs. therefore, it is fine to use MOV_TYPE for PUSH and INT
                // because the argument types and counts are validated.
                //
                //
                // But, I do need to check labels as they are not detected in previous instances
                //
                // I should also allow it to reference CONSTs too, and see if it is talking about a label
                // or a CONST (check labels first)
                //
                // ^^^^ I need to validate that there are no duplicate names across symbol tabels for
                // labels and constants
                "hlt" => (HLT_OP, HLT_TYPE),
                "add" => (ADD_OP, MOV_TYPE),
                "bo" => (BO_OP, B_TYPE),
                "bno" => (BNO_OP, B_TYPE),
                "pop" => (POP_OP, POP_TYPE),
                "div" => (DIV_OP, MOV_TYPE),
                "ret" => (RET_OP, HLT_TYPE),
                "bl" => (BL_OP, B_TYPE),
                "bg" => (BG_OP, B_TYPE),
                "ld" => (LD_OP, LD_TYPE),
                "st" => (ST_OP, ST_TYPE),
                "jmp" => (JMP_OP, B_TYPE),
                "bz" => (BZ_OP, B_TYPE),
                "bnz" => (BNZ_OP, B_TYPE),
                "cmp" => (CMP_OP, MOV_TYPE),
                "nand" => (NAND_OP, MOV_TYPE),
                "push" => (PUSH_OP, MOV_TYPE),
                "int" => (INT_OP, MOV_TYPE),
                "mov" => (MOV_OP, MOV_TYPE),
                "lea" => (LEA_OP, LD_TYPE),
                _ => gen_ice!(
                    "INSTRUCTION MATCH FAILED: {} WAS NOT RECOGNIZED.",
                    ins.name.to_uppercase().magenta()
                ),
            };
            match encode_instruction(fname, &opcode, &ins_class, &ins.args) {
                Ok(v) => encoded_tokens.push(v),
                Err(e) => return Err(e),
            }
        }
        TokenKind::Directive(name) => match name.to_lowercase().as_str() {
            "asciiz" => {
                for letter in next_ins.unwrap().1.get_str().chars() {
                    encoded_tokens.push(letter as i16);
                }
            }
            "word" => {
                encoded_tokens.push(next_ins.unwrap().1.get_value() as i16);
            }
            _ => gen_ice!("DIRECTIVE MATCH FAILED: {name} NOT RECOGNIZED"),
        },
        _ => {}
    }

    Ok(encoded_tokens)
}

use crate::InstructionArgument::*;
fn encode_instruction(
    fname: &String,
    opcode: &i16,
    class: &u8,
    args: &[(InstructionArgument, std::ops::Range<usize>)],
) -> Result<i16, CodeGenError> {
    let lhs = args.first();
    let rhs = args.get(1);
    let mut encoded = 0;
    let l_map = LABEL_MAP.lock().unwrap();
    match *class {
        HLT_TYPE => {
            encoded = opcode << 12;
        }
        MOV_TYPE => {
            encoded = opcode << 12;
            if let Some((Reg(r), _)) = lhs {
                encoded |= (*r as i16) << 9;
            } else {
                gen_ice!("INSTRUCTION ENCODE APPEARS TO HAVE EMPTY MEMORY")
            }
            if let Some((arg, f)) = rhs {
                match arg {
                    Reg(r) => encoded |= *r as i16,
                    IReg(r) => encoded = encoded | (1 << 6) | (*r as i16),
                    Mem(m) => {
                        if let Some(v) = m.content.first() {
                            encoded = encoded | (1 << 7) | (v.0.get_value() as i16);
                        } else {
                            return Err(CodeGenError {
                                file: fname.to_string(),
                                help: None,
                                input: read_file(fname),
                                message: String::from(
                                    "MOV type instruction appears to have empty memory",
                                ),
                                start_pos: f.start,
                                last_pos: f.end,
                            });
                        }
                    }
                    Imm(_) => encoded = encoded | (1 << 8) | arg.get_imm(),
                    _ => gen_ice!("MOV TYPE INSTRUCTION HAS EMPTY/INVALID RHS"),
                }
            } else {
                gen_ice!("MOV TYPE INSTRUCTION APPEARS TO NOT HAVE RHS")
            }
        }
        B_TYPE => {
            encoded = opcode << 11;
            match &lhs.unwrap().0 {
                // safe to unwrap - checked earlier
                Ident(i) => {
                    if let Some((name, span, value)) = l_map.get(i) {
                        if *value >= 1024 {
                            return Err(CodeGenError {
                                file: name.to_string(),
                                help: None,
                                input: read_file(name),
                                message: format!(
                                    "the address of label \"{i}\" cannot fit within 10 bits"
                                ),
                                start_pos: span.start,
                                last_pos: span.end,
                            });
                        } else {
                            encoded |= *value as i16;
                        }
                    } else {
                        return Err(CodeGenError {
                            file: fname.to_string(),
                            help: None,
                            input: read_file(fname),
                            message: format!("cannot find label \"{i}\""),
                            start_pos: args.get(1).unwrap().1.start,
                            last_pos: args.get(1).unwrap().1.end,
                        });
                    }
                }
                Mem(m) => {
                    if let Some(v) = m.content.first() {
                        encoded |= v.0.get_value() as i16; // value limits are checked earlier
                    } else {
                        return Err(CodeGenError {
                            file: fname.to_string(),
                            help: None,
                            input: read_file(fname),
                            message: String::from("Branch instruction memory appears empty"),
                            start_pos: args.first().unwrap().1.start,
                            last_pos: args.first().unwrap().1.end,
                        });
                    }
                }
                IReg(r) => {
                    encoded = encoded | (1 << 10) | (*r as i16);
                }
                _ => gen_ice!("BRANCH INSTRUCTION HAS INVALID LHS"),
            }
        }
        POP_TYPE => {
            encoded = opcode << 12;
            match &lhs.unwrap().0 {
                Ident(i) => {
                    if let Some((name, span, value)) = l_map.get(i) {
                        if *value >= 2048 {
                            return Err(CodeGenError {
                                file: name.to_string(),
                                help: None,
                                input: read_file(name),
                                message: format!(
                                    "the address of label \"{i}\" cannot fit within 11 bits"
                                ),
                                start_pos: span.start,
                                last_pos: span.end,
                            });
                        } else {
                            encoded = encoded | (1 << 11) | (*value as i16);
                        }
                    } else {
                        return Err(CodeGenError {
                            file: fname.to_string(),
                            help: None,
                            input: read_file(fname),
                            message: format!("cannot find label \"{i}\""),
                            start_pos: args.get(1).unwrap().1.start,
                            last_pos: args.get(1).unwrap().1.end,
                        });
                    }
                }

                Mem(m) => {
                    if let Some(v) = m.content.first() {
                        encoded = encoded | (1 << 11) | v.0.get_value() as i16;
                    } else {
                        return Err(CodeGenError {
                            file: fname.to_string(),
                            help: None,
                            input: read_file(fname),
                            message: String::from("POP instruction memory appears empty"),
                            start_pos: args.first().unwrap().1.start,
                            last_pos: args.first().unwrap().1.end,
                        });
                    }
                }

                Reg(r) => encoded |= *r as i16,
                _ => {
                    gen_ice!("POP INSTRUCTION HAS INVALID LHS - THIS SHOULD'VE BEEN CAUGHT EARLIER")
                }
            }
        }
        LD_TYPE => {
            encoded = opcode << 12;
            if let Some((Reg(r), _)) = lhs {
                encoded |= (*r as i16) << 9;
            } else {
                gen_ice!("LD INSTRUCTION LHS DOES NOT APPEAR TO BE REGISTER");
            }
            match &rhs.unwrap().0 {
                Ident(i) => {
                    if let Some((name, span, value)) = l_map.get(i) {
                        if *value >= 512 {
                            return Err(CodeGenError {
                                file: name.to_string(),
                                help: None,
                                input: read_file(name),
                                message: format!(
                                    "the address of label \"{i}\" cannot fit within 9 bits"
                                ),
                                start_pos: span.start,
                                last_pos: span.end,
                            });
                        } else {
                            encoded |= *value as i16;
                        }
                    } else {
                        return Err(CodeGenError {
                            file: fname.to_string(),
                            help: None,
                            input: read_file(fname),
                            message: format!("cannot find label \"{i}\""),
                            start_pos: args.get(1).unwrap().1.start,
                            last_pos: args.get(1).unwrap().1.end,
                        });
                    }
                }

                Mem(m) => {
                    if let Some(v) = m.content.first() {
                        encoded |= v.0.get_value() as i16;
                    } else {
                        return Err(CodeGenError {
                            file: fname.to_string(),
                            help: None,
                            input: read_file(fname),
                            message: String::from("LD/LEA instruction memory appears empty"),
                            start_pos: args.first().unwrap().1.start,
                            last_pos: args.first().unwrap().1.end,
                        });
                    }
                }
                _ => gen_ice!(
                    "LEA/LD INSTRUCTION HAS INVALID RHS - THIS SHOULD'VE BEEN CAUGHT EARLIER"
                ),
            }
        }
        ST_TYPE => match &lhs.unwrap().0 {
            Ident(i) => {
                if let Some((name, span, value)) = l_map.get(i) {
                    if *value >= 256 {
                        return Err(CodeGenError {
                            file: name.to_string(),
                            help: None,
                            input: read_file(name),
                            message: format!(
                                "the address of label \"{i}\" cannot fit within 8 bits"
                            ),
                            start_pos: span.start,
                            last_pos: span.end,
                        });
                    } else {
                        encoded |= (*value as i16) << 3;
                    }
                } else {
                    return Err(CodeGenError {
                        file: fname.to_string(),
                        help: None,
                        input: read_file(fname),
                        message: format!("cannot find label \"{i}\""),
                        start_pos: args.get(1).unwrap().1.start,
                        last_pos: args.get(1).unwrap().1.end,
                    });
                }
            }

            Mem(m) => {
                encoded |= (m.content.first().unwrap().0.get_value() as i16) << 3;
            }

            IReg(r) => {
                encoded = encoded | (1 << 11) | ((*r as i16) << 3);
            }
            _ => gen_ice!("ST INSTRUCTION HAS INVALID LHS - THIS SHOULD'VE BEEN CAUGHT EARLIER"),
        },
        _ => gen_ice!("INVALID INSTRUCTION TYPE IN CODEGEN"),
    }
    Ok(encoded)
}
