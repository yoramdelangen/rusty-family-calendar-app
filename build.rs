use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=assets/icons");
    println!("cargo:rerun-if-changed=src");

    let icon_root = Path::new("assets/icons");
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("embedded_icons.rs");
    let mut icons = used_icons(Path::new("src"));

    icons.sort();
    icons.dedup();

    let mut code = String::from(
        "pub fn embedded_icon(path: &str) -> Option<&'static [u8]> {\n    match path {\n",
    );
    for icon in icons {
        let rel = if icon.ends_with(".svg") {
            icon.clone()
        } else {
            format!("{icon}.svg")
        };
        let abs = icon_root
            .join(&rel)
            .canonicalize()
            .unwrap_or_else(|_| panic!("used icon is missing: {}", icon_root.join(&rel).display()));
        let no_ext = rel.trim_end_matches(".svg");

        code.push_str(&format!(
            "        {:?} | {:?} => Some(include_bytes!({:?})),\n",
            rel,
            no_ext,
            abs.to_string_lossy()
        ));
    }
    code.push_str("        _ => None,\n    }\n}\n");

    fs::write(out, code).unwrap();
}

fn used_icons(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_rs(dir, &mut files);

    files
        .into_iter()
        .filter(|path| path != Path::new("src/icons/mod.rs"))
        .filter_map(|path| fs::read_to_string(path).ok())
        .flat_map(|src| {
            scan_string_arg(&src, "icon(")
                .into_iter()
                .chain(scan_string_arg(&src, "IconInfo::new("))
                .chain(scan_string_in_call(&src, "set_icon_by_name("))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn collect_rs(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

fn scan_string_arg(src: &str, call: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = src;

    while let Some(pos) = rest.find(call) {
        rest = &rest[pos + call.len()..];
        let trimmed = rest.trim_start();
        let Some(after_quote) = trimmed.strip_prefix('"') else {
            continue;
        };
        let Some(end) = after_quote.find('"') else {
            continue;
        };
        found.push(after_quote[..end].to_string());
    }

    found
}

fn scan_string_in_call(src: &str, call: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = src;

    while let Some(pos) = rest.find(call) {
        rest = &rest[pos + call.len()..];

        let Some(end) = matching_paren(rest) else {
            break;
        };

        let args = &rest[..end];
        let mut arg_rest = args;
        while let Some(quote) = arg_rest.find('"') {
            arg_rest = &arg_rest[quote + 1..];
            let Some(end_quote) = arg_rest.find('"') else {
                break;
            };
            found.push(arg_rest[..end_quote].to_string());
            arg_rest = &arg_rest[end_quote + 1..];
        }

        rest = &rest[end + 1..];
    }

    found
}

fn matching_paren(src: &str) -> Option<usize> {
    let mut depth = 1usize;

    for (idx, ch) in src.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }

    None
}
