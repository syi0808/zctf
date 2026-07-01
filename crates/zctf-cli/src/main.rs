use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
};
use zctf_codegen::{Options, generate};

fn main() {
    if let Err(error) = run() {
        eprintln!("zctf: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("codegen") => codegen(args.collect()),
        Some("inspect") => inspect(args.collect()),
        _ => Err("usage: zctf <codegen|inspect> [options]".into()),
    }
}

fn codegen(args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut schemas = vec![];
    let mut out = None;
    let mut emits = "js,ts".to_string();
    let mut runtime_import = "@zctf/runtime".to_string();
    let mut check = false;
    let mut dry_run = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--schema" => {
                index += 1;
                schemas.push(PathBuf::from(
                    args.get(index).ok_or("--schema needs a path")?,
                ));
            }
            "--out" => {
                index += 1;
                out = Some(PathBuf::from(args.get(index).ok_or("--out needs a path")?));
            }
            "--emit" => {
                index += 1;
                emits = args.get(index).ok_or("--emit needs a value")?.clone();
            }
            "--runtime-import" => {
                index += 1;
                runtime_import = args
                    .get(index)
                    .ok_or("--runtime-import needs a value")?
                    .clone();
            }
            "--module" => {
                index += 1;
                if args.get(index).map(String::as_str) != Some("esm") {
                    return Err("only --module esm is supported".into());
                }
            }
            "--check" => check = true,
            "--dry-run" => dry_run = true,
            value if !value.starts_with('-') => schemas.push(PathBuf::from(value)),
            value => return Err(format!("unknown option {value}").into()),
        }
        index += 1;
    }
    if schemas.is_empty() {
        return Err("at least one --schema path is required".into());
    }
    let out = out.ok_or("--out is required")?;
    let paths = collect_paths(&schemas)?;
    let fragments = paths
        .iter()
        .map(zctf_schema::load_fragment)
        .collect::<Result<Vec<_>, _>>()?;
    let assembled = zctf_schema::assemble(fragments)?;
    if assembled.is_empty() {
        return Err("no document schema fragment found".into());
    }
    let emit_values = emits.split(',').collect::<Vec<_>>();
    let options = Options { runtime_import };
    let mut stale = false;
    for schema in assembled {
        let canonical = out.join(format!("{}.schema.json", kebab(&schema.name)));
        let canonical_contents = format!("{}\n", serde_json::to_string_pretty(&schema)?);
        stale |= output(&canonical, &canonical_contents, check, dry_run)?;
        for file in generate(&schema, &emit_values, &options)? {
            stale |= output(&out.join(file.name), &file.contents, check, dry_run)?;
        }
    }
    if stale {
        return Err("generated output is stale".into());
    }
    Ok(())
}

fn output(
    path: &Path,
    contents: &str,
    check: bool,
    dry_run: bool,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    if check {
        return Ok(fs::read_to_string(path)
            .map(|s| s != contents)
            .unwrap_or(true));
    }
    if dry_run {
        println!("{}", path.display());
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)?;
    }
    Ok(false)
}

fn collect_paths(
    inputs: &[PathBuf],
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    let mut found = BTreeSet::new();
    for input in inputs {
        let input_text = input.to_string_lossy();
        if input_text.contains(['*', '?', '[']) {
            for entry in glob::glob(&input_text)? {
                found.insert(entry?);
            }
        } else if input.is_dir() {
            for entry in fs::read_dir(input)? {
                let path = entry?.path();
                if path.extension().and_then(|x| x.to_str()) == Some("json") {
                    found.insert(path);
                }
            }
        } else {
            found.insert(input.clone());
        }
    }
    Ok(found.into_iter().collect())
}

fn inspect(args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = args.first().ok_or("usage: zctf inspect <schema>")?;
    let value: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn kebab(value: &str) -> String {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                out.push('-');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}
