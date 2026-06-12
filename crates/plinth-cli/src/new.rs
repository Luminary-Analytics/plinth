//! `plinth new`: scaffold a runnable game project.

use std::path::PathBuf;

struct TemplateFile {
    relative_path: &'static str,
    content: &'static str,
}

const TEMPLATE: &[TemplateFile] = &[
    TemplateFile {
        relative_path: "Cargo.toml",
        content: include_str!("../templates/Cargo.toml.tmpl"),
    },
    TemplateFile {
        relative_path: "src/main.rs",
        content: include_str!("../templates/main.rs.tmpl"),
    },
    TemplateFile {
        relative_path: "scenes/arena.scene.json",
        content: include_str!("../templates/arena.scene.json.tmpl"),
    },
    TemplateFile {
        relative_path: "README.md",
        content: include_str!("../templates/README.md.tmpl"),
    },
    TemplateFile {
        relative_path: ".gitignore",
        content: "/target\n",
    },
];

/// Create `./<name>` from the template. Returns the project directory.
pub fn create_project(name: &str) -> Result<PathBuf, String> {
    if !valid_project_name(name) {
        return Err(format!(
            "invalid project name `{name}`: use lowercase letters, digits, `-` and `_`, starting with a letter"
        ));
    }

    let root = PathBuf::from(name);
    if root.exists()
        && root
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(true)
    {
        return Err(format!(
            "directory `{}` already exists and is not empty",
            root.display()
        ));
    }

    for file in TEMPLATE {
        let path = root.join(file.relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("cannot create {}: {err}", parent.display()))?;
        }
        let content = file.content.replace("{{name}}", name);
        std::fs::write(&path, content)
            .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    }

    Ok(root)
}

fn valid_project_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}
