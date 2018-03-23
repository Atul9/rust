//! Generate rust code from varlink interface definition files

extern crate varlink_parser;

use std::env;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::io;
use std::io::Error as IOError;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::exit;
use std::result::Result;
use varlink_parser::{Interface, VStruct, VStructOrEnum, VType, VTypeExt, Varlink};

type EnumVec<'a> = Vec<(String, Vec<String>)>;
type StructVec<'a> = Vec<(String, &'a VStruct<'a>)>;

trait ToRust<'short, 'long: 'short> {
    fn to_rust(
        &'long self,
        parent: &str,
        enumvec: &mut EnumVec,
        structvec: &mut StructVec<'short>,
    ) -> Result<String, ToRustError>;
}

#[derive(Debug)]
enum ToRustError {
    IoError(IOError),
}

impl Error for ToRustError {
    fn description(&self) -> &str {
        match *self {
            ToRustError::IoError(_) => "an I/O error occurred",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match self {
            &ToRustError::IoError(ref err) => Some(&*err as &Error),
        }
    }
}

impl From<IOError> for ToRustError {
    fn from(err: IOError) -> ToRustError {
        ToRustError::IoError(err)
    }
}

impl fmt::Display for ToRustError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())?;
        Ok(())
    }
}

impl<'short, 'long: 'short> ToRust<'short, 'long> for VType<'long> {
    fn to_rust(
        &'long self,
        parent: &str,
        enumvec: &mut EnumVec,
        structvec: &mut StructVec<'short>,
    ) -> Result<String, ToRustError> {
        match self {
            &VType::Bool(_) => Ok("bool".into()),
            &VType::Int(_) => Ok("i64".into()),
            &VType::Float(_) => Ok("f64".into()),
            &VType::VString(_) => Ok("String".into()),
            &VType::VTypename(v) => Ok(v.into()),
            &VType::VEnum(ref v) => {
                enumvec.push((
                    parent.into(),
                    Vec::from_iter(v.elts.iter().map(|s| String::from(*s))),
                ));
                Ok(format!("{}", parent).into())
            }
            &VType::VStruct(ref v) => {
                structvec.push((String::from(parent), v.as_ref()));
                Ok(format!("{}", parent).into())
            }
        }
    }
}

impl<'short, 'long: 'short> ToRust<'short, 'long> for VTypeExt<'long> {
    fn to_rust(
        &'long self,
        parent: &str,
        enumvec: &mut EnumVec,
        structvec: &mut StructVec<'short>,
    ) -> Result<String, ToRustError> {
        let v = self.vtype.to_rust(parent, enumvec, structvec)?;

        if self.isarray {
            Ok(format!("Vec<{}>", v).into())
        } else {
            Ok(v.into())
        }
    }
}

fn to_snake_case(mut str: &str) -> String {
    let mut words = vec![];
    // Preserve leading underscores
    str = str.trim_left_matches(|c: char| {
        if c == '_' {
            words.push(String::new());
            true
        } else {
            false
        }
    });
    for s in str.split('_') {
        let mut last_upper = false;
        let mut buf = String::new();
        if s.is_empty() {
            continue;
        }
        for ch in s.chars() {
            if !buf.is_empty() && buf != "'" && ch.is_uppercase() && !last_upper {
                words.push(buf);
                buf = String::new();
            }
            last_upper = ch.is_uppercase();
            buf.extend(ch.to_lowercase());
        }
        words.push(buf);
    }
    words.join("_")
}

fn is_rust_keyword(v: &str) -> bool {
    match v {
        "abstract" | "alignof" | "as" | "become" | "box" | "break" | "const" | "continue"
        | "crate" | "do" | "else" | "enum" | "extern" | "false" | "final" | "fn" | "for" | "if"
        | "impl" | "in" | "let" | "loop" | "macro" | "match" | "mod" | "move" | "mut"
        | "offsetof" | "override" | "priv" | "proc" | "pub" | "pure" | "ref" | "return"
        | "Self" | "self" | "sizeof" | "static" | "struct" | "super" | "trait" | "true"
        | "type" | "typeof" | "unsafe" | "unsized" | "use" | "virtual" | "where" | "while"
        | "yield" => true,
        _ => false,
    }
}

fn replace_if_rust_keyword(v: &str) -> String {
    if is_rust_keyword(v) {
        String::from(v) + "_"
    } else {
        String::from(v)
    }
}

fn replace_if_rust_keyword_annotate(v: &str, out: &mut String, prefix: &str) -> String {
    if is_rust_keyword(v) {
        *out += prefix;
        *out += format!("#[serde(rename = \"{}\")] ", v).as_ref();
        String::from(v) + "_"
    } else {
        *out += prefix;
        String::from(v)
    }
}

trait InterfaceToRust {
    fn to_rust(&self, description: &String) -> Result<String, ToRustError>;
}

impl<'a> InterfaceToRust for Interface<'a> {
    fn to_rust(&self, description: &String) -> Result<String, ToRustError> {
        let mut out: String = "".to_owned();
        let mut enumvec = EnumVec::new();
        let mut structvec = StructVec::new();

        for t in self.typedefs.values() {
            match t.elt {
                VStructOrEnum::VStruct(ref v) => {
                    out += "#[derive(Serialize, Deserialize, Debug, Default)]\n";
                    out += format!("pub struct {} {{\n", replace_if_rust_keyword(t.name)).as_ref();
                    for e in &v.elts {
                        out += "    #[serde(skip_serializing_if = \"Option::is_none\")]";
                        out += format!(
                            "pub {}: Option<{}>,\n",
                            replace_if_rust_keyword_annotate(e.name, &mut out, " "),
                            e.vtype.to_rust(
                                format!("{}_{}", t.name, e.name).as_ref(),
                                &mut enumvec,
                                &mut structvec
                            )?
                        ).as_ref();
                    }
                }
                VStructOrEnum::VEnum(ref v) => {
                    out += "#[derive(Serialize, Deserialize, Debug)]\n";
                    out += format!("pub enum {} {{\n", t.name).as_ref();
                    let mut iter = v.elts.iter();
                    for elt in iter {
                        out += format!(
                            "{},\n",
                            replace_if_rust_keyword_annotate(elt, &mut out, "    ")
                        ).as_ref();
                    }
                    out += "\n";
                }
            }
            out += "}\n\n";
        }

        for t in self.methods.values() {
            if t.output.elts.len() > 0 {
                out += "#[derive(Serialize, Deserialize, Debug)]\n";
                out += format!("struct _{}Reply {{\n", t.name).as_ref();
                for e in &t.output.elts {
                    out += "    #[serde(skip_serializing_if = \"Option::is_none\")]";
                    out += format!(
                        "{}: Option<{}>,\n",
                        replace_if_rust_keyword_annotate(e.name, &mut out, " "),
                        e.vtype.to_rust(
                            format!("{}Reply_{}", t.name, e.name).as_ref(),
                            &mut enumvec,
                            &mut structvec
                        )?
                    ).as_ref();
                }
                out += "}\n\n";
                out += format!("impl varlink::VarlinkReply for _{}Reply {{}}\n\n", t.name).as_ref();
            }

            if t.input.elts.len() > 0 {
                out += "#[derive(Serialize, Deserialize, Debug)]\n";
                out += format!("struct _{}Args {{\n", t.name).as_ref();
                for e in &t.input.elts {
                    out += "    #[serde(skip_serializing_if = \"Option::is_none\")]";
                    out += format!(
                        "{}: Option<{}>,\n",
                        replace_if_rust_keyword_annotate(e.name, &mut out, " "),
                        e.vtype.to_rust(
                            format!("{}Args_{}", t.name, e.name).as_ref(),
                            &mut enumvec,
                            &mut structvec
                        )?
                    ).as_ref();
                }
                out += "}\n\n";
            }
        }

        for t in self.errors.values() {
            if t.parm.elts.len() > 0 {
                out += "#[derive(Serialize, Deserialize, Debug)]\n";
                out += format!("struct _{}Args {{\n", t.name).as_ref();
                for e in &t.parm.elts {
                    out += "    #[serde(skip_serializing_if = \"Option::is_none\")]";
                    out += format!(
                        "{}: Option<{}>,\n",
                        replace_if_rust_keyword_annotate(e.name, &mut out, " "),
                        e.vtype.to_rust(
                            format!("{}Args_{}", t.name, e.name).as_ref(),
                            &mut enumvec,
                            &mut structvec
                        )?
                    ).as_ref();
                }
                out += "}\n\n";
            }
        }

        loop {
            let mut nstructvec = StructVec::new();
            for (name, v) in structvec.drain(..) {
                out += "#[derive(Serialize, Deserialize, Debug, Default)]\n";
                out += format!("pub struct {} {{\n", replace_if_rust_keyword(&name)).as_ref();
                for e in &v.elts {
                    out += "    #[serde(skip_serializing_if = \"Option::is_none\")]";
                    out += format!(
                        "pub {}: Option<{}>,\n",
                        replace_if_rust_keyword_annotate(e.name, &mut out, " "),
                        e.vtype
                            .to_rust(
                                format!("{}_{}", name, e.name).as_ref(),
                                &mut enumvec,
                                &mut nstructvec
                            )
                            .unwrap()
                    ).as_ref();
                }
                out += "}\n\n";
            }
            for (name, v) in enumvec.drain(..) {
                out += format!(
                    "#[derive(Serialize, Deserialize, Debug)]\n\
                     pub enum {} {{\n",
                    replace_if_rust_keyword(name.as_str())
                ).as_ref();
                let mut iter = v.iter();
                for elt in iter {
                    out += format!(
                        "{},\n",
                        replace_if_rust_keyword_annotate(elt, &mut out, "    ")
                    ).as_ref();
                }
                out += "\n}\n\n";
            }

            if nstructvec.len() == 0 {
                break;
            }
            structvec = nstructvec;
        }

        out += "pub trait _CallErr: varlink::CallTrait {\n";
        if self.errors.len() > 0 {
            for t in self.errors.values() {
                let mut inparms: String = "".to_owned();
                let mut innames: String = "".to_owned();
                if t.parm.elts.len() > 0 {
                    for e in &t.parm.elts {
                        inparms += format!(
                            ", {}: Option<{}>",
                            replace_if_rust_keyword(e.name),
                            e.vtype.to_rust(
                                format!("{}Args_{}", t.name, e.name).as_ref(),
                                &mut enumvec,
                                &mut structvec
                            )?
                        ).as_ref();
                        innames += format!("{}, ", replace_if_rust_keyword(e.name)).as_ref();
                    }
                    innames.pop();
                    innames.pop();
                }
                out += format!(
                    r#"    fn reply_{}(&mut self{}) -> io::Result<()> {{
        self.reply_struct(varlink::Reply::error(
            "{}.{}".into(),
"#,
                    to_snake_case(t.name),
                    inparms,
                    self.name,
                    t.name,
                ).as_ref();
                if t.parm.elts.len() > 0 {
                    out += format!(
                        "            Some(serde_json::to_value(_{}Args {{ {} }}).unwrap()),",
                        t.name, innames
                    ).as_ref();
                } else {
                    out += "        None,\n";
                }

                out += r#"
        ))
    }
"#;
            }
        }
        out += "}\n\nimpl<'a> _CallErr for varlink::Call<'a> {}\n\n";

        for t in self.methods.values() {
            let mut inparms: String = "".to_owned();
            let mut innames: String = "".to_owned();
            if t.output.elts.len() > 0 {
                for e in &t.output.elts {
                    inparms += format!(
                        ", {}: Option<{}>",
                        replace_if_rust_keyword(e.name),
                        e.vtype.to_rust(
                            format!("{}Reply_{}", t.name, e.name).as_ref(),
                            &mut enumvec,
                            &mut structvec
                        )?
                    ).as_ref();
                    innames += format!("{}, ", replace_if_rust_keyword(e.name)).as_ref();
                }
                innames.pop();
                innames.pop();
            }
            out += format!("pub trait _Call{}: _CallErr {{\n", t.name).as_ref();
            out += format!("    fn reply(&mut self{}) -> io::Result<()> {{\n", inparms).as_ref();
            if t.output.elts.len() > 0 {
                out += format!(
                    "        self.reply_struct(_{}Reply {{ {} }}.into())\n",
                    t.name, innames
                ).as_ref();
            } else {
                out += "        self.reply_struct(varlink::Reply::parameters(None))\n";
            }
            out += format!(
                "    }}\n}}\n\nimpl<'a> _Call{} for varlink::Call<'a> {{}}\n\n",
                t.name
            ).as_ref();
        }

        out += "pub trait VarlinkInterface {\n";
        for t in self.methods.values() {
            let mut inparms: String = "".to_owned();
            if t.input.elts.len() > 0 {
                for e in &t.input.elts {
                    inparms += format!(
                        ", {}: Option<{}>",
                        replace_if_rust_keyword(e.name),
                        e.vtype.to_rust(
                            format!("{}Args_{}", t.name, e.name).as_ref(),
                            &mut enumvec,
                            &mut structvec
                        )?
                    ).as_ref();
                }
            }

            out += format!(
                "    fn {}(&self, call: &mut _Call{}{}) -> io::Result<()>;\n",
                to_snake_case(t.name),
                t.name,
                inparms
            ).as_ref();
        }

        out += r#"    fn call_upgraded(&self, _call: &mut varlink::Call) -> io::Result<()> {
        Ok(())
    }
}
"#;

        out += format!(
            r####"
pub struct _InterfaceProxy {{
    inner: Box<VarlinkInterface + Send + Sync>,
}}

pub fn new(inner: Box<VarlinkInterface + Send + Sync>) -> _InterfaceProxy {{
    _InterfaceProxy {{ inner }}
}}

impl varlink::Interface for _InterfaceProxy {{
    fn get_description(&self) -> &'static str {{
        r#"
{}
"#
    }}

    fn get_name(&self) -> &'static str {{
        "{}"
    }}

"####,
            description, self.name
        ).as_ref();

        out += r#"    fn call_upgraded(&self, call: &mut varlink::Call) -> io::Result<()> {
        self.inner.call_upgraded(call)
    }

    fn call(&self, call: &mut varlink::Call) -> io::Result<()> {
        let req = call.request.unwrap();
        match req.method.as_ref() {
"#;

        for t in self.methods.values() {
            let mut inparms: String = "".to_owned();
            for e in &t.input.elts {
                inparms += format!(", args.{}", replace_if_rust_keyword(e.name)).as_ref();
            }

            out += format!("            \"{}.{}\" => {{", self.name, t.name).as_ref();
            if t.input.elts.len() > 0 {
                out +=
                    format!(
                        concat!("\n                if let Some(args) = req.parameters.clone() {{\n",
"                    let args: _{}Args = serde_json::from_value(args)?;\n",
"                    return self.inner.{}(call as &mut _Call{}{});\n",
"                }} else {{\n",
"                    return call.reply_invalid_parameter(None);\n",
"                }}\n",
"            }}\n"),
                        t.name,
                        to_snake_case(t.name), t.name,
                        inparms
                    ).as_ref();
            } else {
                out += format!(
                    "\n                return self.inner.{}(call as &mut _Call{});\n            }}\n",
                    to_snake_case(t.name), t.name
                ).as_ref();
            }
        }
        out += concat!(
            "\n",
            "            m => {\n",
            "                return call.reply_method_not_found(Some(String::from(m)));\n",
            "            }\n",
            "        }\n",
            "    }\n",
            "}"
        );

        Ok(out)
    }
}

/// `generate` reads a varlink interface definition from `reader` and writes
/// the rust code to `writer`.
pub fn generate(reader: &mut Read, writer: &mut Write) -> io::Result<()> {
    let mut buffer = String::new();

    reader.read_to_string(&mut buffer)?;

    let vr = Varlink::from_string(&buffer);

    if let Err(e) = vr {
        eprintln!("{}", e);
        exit(1);
    }

    match vr.unwrap().interface.to_rust(&buffer) {
        Ok(out) => {
            writeln!(
                writer,
                r#"//! DO NOT EDIT
//! This file is automatically generated by the varlink rust generator

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::io;

use varlink;
use serde_json;
use varlink::CallTrait;


{}"#,
                out
            )?;
        }
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    }

    Ok(())
}

/// cargo build helper function
///
/// `cargo_build` is used in a `build.rs` program to build the rust code
/// from a varlink interface definition.
///
/// Errors are emitted to stderr and terminate the process.
///
///# Examples
///
///```rust,no_run
///extern crate varlink;
///
///fn main() {
///    varlink::generator::cargo_build("src/org.example.ping.varlink");
///}
///```
///
pub fn cargo_build<T: AsRef<Path> + ?Sized>(input_path: &T) {
    let input_path = input_path.as_ref();

    let out_dir: PathBuf = env::var_os("OUT_DIR").unwrap().into();
    let rust_path = out_dir
        .join(input_path.file_name().unwrap())
        .with_extension("rs");

    let writer: &mut Write = &mut (File::create(&rust_path).unwrap());

    let reader: &mut Read = &mut (File::open(input_path).unwrap_or_else(|e| {
        eprintln!(
            "Could not read varlink input file `{}`: {}",
            input_path.display(),
            e
        );
        exit(1);
    }));

    if let Err(e) = generate(reader, writer) {
        eprintln!(
            "Could not generate rust code from varlink file `{}`: {}",
            input_path.display(),
            e
        );
        exit(1);
    }

    println!("cargo:rerun-if-changed={}", input_path.display());
}

/// cargo build helper function
///
/// `cargo_build_tosource` is used in a `build.rs` program to build the rust code
/// from a varlink interface definition. This function saves the rust code
/// in the same directory as the varlink file. The name is the name of the varlink file
/// and "." replaced with "_" and of course ending with ".rs".
///
/// Use this, if you are using an IDE with code completion, as most cannot cope with
/// `include!(concat!(env!("OUT_DIR"), "<varlink_file>"));`
///
/// Set `rustfmt` to `true`, if you want the generator to run rustfmt on the generated
/// code. This might be good practice to avoid large changes after a global `cargo fmt` run.
///
/// Errors are emitted to stderr and terminate the process.
///
///# Examples
///
///```rust,no_run
///extern crate varlink;
///
///fn main() {
///    varlink::generator::cargo_build_tosource("src/org.example.ping.varlink", true);
///}
///```
///
pub fn cargo_build_tosource<T: AsRef<Path> + ?Sized>(input_path: &T, rustfmt: bool) {
    let input_path = input_path.as_ref();
    let noextension = input_path.with_extension("");
    let newfilename = noextension
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .replace(".", "_");
    let rust_path = input_path
        .parent()
        .unwrap()
        .join(Path::new(&newfilename).with_extension("rs"));

    eprintln!("{}", rust_path.display());

    let writer: &mut Write = &mut (File::create(&rust_path).unwrap());

    let reader: &mut Read = &mut (File::open(input_path).unwrap_or_else(|e| {
        eprintln!(
            "Could not read varlink input file `{}`: {}",
            input_path.display(),
            e
        );
        exit(1);
    }));

    if let Err(e) = generate(reader, writer) {
        eprintln!(
            "Could not generate rust code from varlink file `{}`: {}",
            input_path.display(),
            e
        );
        exit(1);
    }

    if rustfmt {
        if let Err(e) = Command::new("rustfmt")
            .arg(rust_path.to_str().unwrap())
            .output()
        {
            eprintln!(
                "Could not run rustfmt on file `{}` {}",
                rust_path.display(),
                e
            );
            exit(1);
        }
    }

    println!("cargo:rerun-if-changed={}", input_path.display());
}
